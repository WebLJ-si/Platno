import { createHash, randomBytes, scryptSync, timingSafeEqual } from 'node:crypto';
import { SignJWT, jwtVerify, createRemoteJWKSet } from 'jose';
const googleKeys=createRemoteJWKSet(new URL('https://www.googleapis.com/oauth2/v3/certs'));
export function settings(env=process.env){
 const origin=new URL(env.APP_URL||'http://127.0.0.1:8790');
 if(origin.pathname!=='/'||origin.search||origin.hash||origin.username||origin.password)throw Error('APP_URL must be an origin');
 if(env.VERCEL&&origin.protocol!=='https:')throw Error('HTTPS required');
 return {origin:origin.origin,secure:origin.protocol==='https:',clientId:env.GOOGLE_CLIENT_ID,clientSecret:env.GOOGLE_CLIENT_SECRET,owner:(env.OWNER_EMAIL||'').trim().toLowerCase(),secret:env.SESSION_SECRET,codeHash:(env.DASHBOARD_CODE_HASH||'').trim()};
}
function key(config){if(!config.secret||config.secret.length<32)throw Error('Authentication not configured');return new TextEncoder().encode(config.secret)}
export function cookies(req){const result=Object.create(null);for(const part of (req.headers.cookie||'').split(';')){const at=part.indexOf('=');if(at>0)result[part.slice(0,at).trim()]=part.slice(at+1).trim()}return result}

export function cookie(config,name,value,maxAge){return `${name}=${value}; HttpOnly; SameSite=Lax; Path=/; Max-Age=${maxAge}${config.secure?'; Secure':''}`}
export async function sign(payload,config,audience,seconds){return new SignJWT(payload).setProtectedHeader({alg:'HS256'}).setIssuer('platno-observatory').setAudience(audience).setIssuedAt().setExpirationTime(`${seconds}s`).sign(key(config))}
export async function verify(token,config,audience){return (await jwtVerify(token,key(config),{algorithms:['HS256'],issuer:'platno-observatory',audience})).payload}
export async function ownerSession(req,config){try{let payload=await verify(cookies(req).platno_owner,config,'dashboard');return !!config.owner&&payload.sub==='owner'&&payload.ownerHash===ownerHash(config)}catch{return false}}
function ownerHash(config){return createHash('sha256').update(config.owner).digest('hex')}
export function ownerAllowed(claims,config){return !!config.owner&&claims.email_verified===true&&typeof claims.email==='string'&&claims.email.trim().toLowerCase()===config.owner&&typeof claims.sub==='string'&&claims.sub.length>0}
export async function login(config){
 if(!config.clientId||!config.clientSecret||!config.owner)throw Error('Google sign-in not configured');
 const state=randomBytes(24).toString('hex'),nonce=randomBytes(24).toString('hex'),verifier=randomBytes(32).toString('base64url');
 const token=await sign({state,nonce,verifier},config,'oauth',600);
 const url=new URL('https://accounts.google.com/o/oauth2/v2/auth');
 for(const [k,v] of Object.entries({client_id:config.clientId,redirect_uri:config.origin+'/api/auth/callback',response_type:'code',scope:'openid email',state,nonce,code_challenge:createHash('sha256').update(verifier).digest('base64url'),code_challenge_method:'S256',prompt:'select_account'}))url.searchParams.set(k,v);
 return {url:url.href,cookie:cookie(config,'platno_oauth',token,600)};
}
export async function callback(req,params,config,{fetcher=fetch,keys=googleKeys}={}){
 const pending=await verify(cookies(req).platno_oauth,config,'oauth');
 const state=params.get('state')||'';
 if(typeof pending.state!=='string'||state.length!==pending.state.length||!timingSafeEqual(Buffer.from(state),Buffer.from(pending.state))||!params.get('code'))throw Error('Invalid OAuth state');
 const response=await fetcher('https://oauth2.googleapis.com/token',{method:'POST',headers:{'Content-Type':'application/x-www-form-urlencoded'},body:new URLSearchParams({client_id:config.clientId,client_secret:config.clientSecret,code:params.get('code'),code_verifier:pending.verifier,redirect_uri:config.origin+'/api/auth/callback',grant_type:'authorization_code'}),signal:AbortSignal.timeout(8000)});
 if(!response.ok)throw Error('Google token exchange failed');
 const {id_token}=await response.json();
 const {payload}=await jwtVerify(id_token,keys,{algorithms:['RS256'],issuer:['https://accounts.google.com','accounts.google.com'],audience:config.clientId});
 if(payload.nonce!==pending.nonce||!ownerAllowed(payload,config)||(payload.azp&&payload.azp!==config.clientId))throw Error('Account not allowed');
 return cookie(config,'platno_owner',await sign({sub:'owner',ownerHash:ownerHash(config)},config,'dashboard',28800),28800);
}
// Simple access code: only a salted scrypt hash is configured (DASHBOARD_CODE_HASH); the code itself is never stored.
export function hashCode(code){const salt=randomBytes(16);return 'scrypt$'+salt.toString('base64url')+'$'+scryptSync(String(code),salt,32,{N:16384,r:8,p:1}).toString('base64url')}
export function verifyCode(code,stored){try{const [kind,salt,hash]=String(stored||'').split('$');if(kind!=='scrypt'||!salt||!hash)return false;const expected=Buffer.from(hash,'base64url'),got=scryptSync(String(code),Buffer.from(salt,'base64url'),expected.length,{N:16384,r:8,p:1});return got.length===expected.length&&timingSafeEqual(got,expected)}catch{return false}}
export function codeConfigured(config){return /^scrypt\$[\w-]+\$[\w-]+$/.test(config.codeHash||'')}
const codeStamp=config=>createHash('sha256').update(config.codeHash||'').digest('hex').slice(0,24);
export async function codeLogin(code,config){if(!codeConfigured(config)||typeof code!=='string'||code.length<1||code.length>64||!verifyCode(code,config.codeHash))throw Error('Wrong code');return cookie(config,'platno_viewer',await sign({sub:'viewer',stamp:codeStamp(config)},config,'viewer',28800),28800)}
export async function viewerSession(req,config){try{const payload=await verify(cookies(req).platno_viewer,config,'viewer');return codeConfigured(config)&&payload.sub==='viewer'&&payload.stamp===codeStamp(config)}catch{return false}}
export function sameOrigin(req,config){return req.headers.origin===config.origin&&req.headers['sec-fetch-site']!=='cross-site'}
