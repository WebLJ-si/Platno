import { randomBytes,createHash } from 'node:crypto';
import { parseReport } from '../lib/schema.mjs';
import { settings,ownerSession,viewerSession,codeLogin,codeConfigured,login,callback,cookie,sameOrigin } from '../lib/auth.mjs';
import { Store } from '../lib/store.mjs';
export const config={api:{bodyParser:false}};
export function createHandler({env=process.env,storeFactory=()=>new Store(env),auth={ownerSession,viewerSession,codeLogin,login,callback}}={}){
 return async function handler(req,res){
  res.setHeader('Cache-Control','no-store');res.setHeader('X-Content-Type-Options','nosniff');
  const respond=(code,data)=>{res.statusCode=code;res.setHeader('Content-Type','application/json');res.end(JSON.stringify(data))};
  const redirect=url=>{res.statusCode=302;res.setHeader('Location',url);res.end()};
  let cfg;try{cfg=settings(env)}catch{return respond(503,{error:'Server configuration required'})}
  let url=new URL(req.url,cfg.origin),route=url.searchParams.get('route')||url.pathname.replace(/^\/api\/?/,'');
  try{
   if(route==='health'&&req.method==='GET')return respond(200,{ok:true,service:'platno-observatory'});
   if(route==='auth/login'&&req.method==='GET'){try{let result=await auth.login(cfg);res.setHeader('Set-Cookie',result.cookie);return redirect(result.url)}catch{return respond(503,{error:'Google sign-in is not configured yet'})}}
   if(route==='auth/callback'&&req.method==='GET'){
    res.setHeader('Set-Cookie',cookie(cfg,'platno_oauth','',0));
    try{const session=await auth.callback(req,url.searchParams,cfg);res.setHeader('Set-Cookie',[cookie(cfg,'platno_oauth','',0),session]);return redirect('/')}catch{return respond(403,{error:'Sign-in denied. Only the configured owner account has access.'})}
   }
   if(route==='auth/methods'&&req.method==='GET')return respond(200,{google:!!(cfg.clientId&&cfg.clientSecret&&cfg.owner),code:codeConfigured(cfg)});
   if(route==='auth/code'&&req.method==='POST'){
    if(!sameOrigin(req,cfg))return respond(403,{error:'Invalid origin'});
    if(!codeConfigured(cfg))return respond(503,{error:'Code sign-in is not configured'});
    let body;try{let raw=req.body!==undefined?(typeof req.body==='string'?req.body:JSON.stringify(req.body)):'';if(req.body===undefined)for await(const chunk of req){raw+=chunk.toString();if(raw.length>1024)return respond(413,{error:'Too large'})}body=JSON.parse(raw||'{}')}catch{return respond(400,{error:'Invalid request'})}
    const ip=String(req.headers['x-vercel-forwarded-for']||req.socket?.remoteAddress||'unknown').split(',')[0].trim();
    if(!await storeFactory().loginAttempt(ip))return respond(429,{error:'Too many attempts. Try again in 15 minutes.'});
    try{res.setHeader('Set-Cookie',await auth.codeLogin(body.code,cfg));return respond(200,{ok:true})}catch{return respond(401,{error:'Wrong code'})}
   }
   if(route==='ingest'&&req.method==='POST'){
    // No cookie, OAuth token, report body or raw client IP is logged or persisted.
    const declared=Number(req.headers['content-length']||0);if(declared>180000)return respond(413,{error:'Report too large'});
    let raw='',bytes=0;try{if(req.body!==undefined){raw=typeof req.body==='string'?req.body:Buffer.isBuffer(req.body)?req.body.toString():JSON.stringify(req.body);if(Buffer.byteLength(raw)>180000)return respond(413,{error:'Report too large'})}else for await(const chunk of req){bytes+=chunk.length;if(bytes>180000)return respond(413,{error:'Report too large'});raw+=chunk.toString()}}catch{return respond(400,{error:'Incomplete report'})}
    let report;try{report=parseReport(JSON.parse(raw))}catch{return respond(400,{error:'Invalid report schema'})}
    const store=storeFactory();const ip=String(req.headers['x-vercel-forwarded-for']||req.socket?.remoteAddress||'unknown').split(',')[0].trim();
    if(!await store.rateLimit(ip))return respond(429,{error:'Reporting limit reached'});
    report.id=randomBytes(16).toString('hex');report.receivedAt=new Date().toISOString();
    report.fingerprint=createHash('sha256').update(JSON.stringify([report.kind,report.code,report.scene,report.frames,report.route,report.status,report.kind==='ui_issue'?Math.round((report.context.width||0)/100):undefined])).digest('hex').slice(0,20);
    await store.save(report);return respond(202,{accepted:true});
   }
   const owner=await auth.ownerSession(req,cfg),viewer=!owner&&await auth.viewerSession(req,cfg);
   if(!owner&&!viewer)return respond(401,{error:'Sign-in required'});
   if(route==='auth/me'&&req.method==='GET')return respond(200,{owner,role:owner?'owner':'viewer'});
   if(route==='auth/logout'&&req.method==='POST'){if(!sameOrigin(req,cfg))return respond(403,{error:'Invalid origin'});res.setHeader('Set-Cookie',[cookie(cfg,'platno_owner','',0),cookie(cfg,'platno_viewer','',0)]);return respond(200,{ok:true})}
   const store=storeFactory();
   if(route==='reports'&&req.method==='GET'){const reports=await store.list(),marks=store.marks?await store.marks(reports.map(r=>r.fingerprint)):[];reports.forEach((r,i)=>{r.status=marks[i]||'open'});return respond(200,{reports,sessions:await store.sessions(),retentionDays:7,window:'Latest 100 reports, not lifetime totals'})}
   if(route==='report'&&req.method==='GET'){const id=url.searchParams.get('id')||'';if(!/^[a-f0-9]{32}$/.test(id))return respond(400,{error:'Invalid report ID'});const report=await store.get(id);if(report&&store.marks)report.status=(await store.marks([report.fingerprint]))[0]||'open';return respond(report?200:404,report||{error:'Report expired or deleted'})}
   if(route==='mark'&&req.method==='POST'){
    if(!sameOrigin(req,cfg))return respond(403,{error:'Invalid origin'});
    const id=url.searchParams.get('id')||'',status=url.searchParams.get('status')||'';if(!/^[a-f0-9]{32}$/.test(id)||!['open','seen','fixed'].includes(status))return respond(400,{error:'Invalid report or status'});
    const report=await store.get(id);if(!report)return respond(404,{error:'Report expired or deleted'});await store.mark(report.fingerprint,status);return respond(200,{ok:true,status});
   }
   if((route==='delete'||route==='purge')&&req.method==='POST'){
    if(!sameOrigin(req,cfg))return respond(403,{error:'Invalid origin'});
    if(!owner)return respond(403,{error:'Deleting needs the owner account'});
    if(route==='purge')await store.purge();else{const id=url.searchParams.get('id')||'';if(!/^[a-f0-9]{32}$/.test(id))return respond(400,{error:'Invalid report ID'});await store.delete(id)}
    return respond(200,{ok:true});
   }
   return respond(404,{error:'Not found'});
  }catch{return respond(503,{error:'Service unavailable. No diagnostic body was logged.'})}
 }
}
export default createHandler();
