import { randomBytes,createHash } from 'node:crypto';
import { parseReport } from '../lib/schema.mjs';
import { settings,ownerSession,login,callback,cookie,sameOrigin } from '../lib/auth.mjs';
import { Store } from '../lib/store.mjs';
export const config={api:{bodyParser:false}};
export function createHandler({env=process.env,storeFactory=()=>new Store(env),auth={ownerSession,login,callback}}={}){
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
   if(!await auth.ownerSession(req,cfg))return respond(401,{error:'Owner sign-in required'});
   if(route==='auth/me'&&req.method==='GET')return respond(200,{owner:true});
   if(route==='auth/logout'&&req.method==='POST'){if(!sameOrigin(req,cfg))return respond(403,{error:'Invalid origin'});res.setHeader('Set-Cookie',cookie(cfg,'platno_owner','',0));return respond(200,{ok:true})}
   const store=storeFactory();
   if(route==='reports'&&req.method==='GET')return respond(200,{reports:await store.list(),sessions:await store.sessions(),retentionDays:7,window:'Latest 100 reports, not lifetime totals'});
   if(route==='report'&&req.method==='GET'){const id=url.searchParams.get('id')||'';if(!/^[a-f0-9]{32}$/.test(id))return respond(400,{error:'Invalid report ID'});const report=await store.get(id);return respond(report?200:404,report||{error:'Report expired or deleted'})}
   if((route==='delete'||route==='purge')&&req.method==='POST'){
    if(!sameOrigin(req,cfg))return respond(403,{error:'Invalid origin'});
    if(route==='purge')await store.purge();else{const id=url.searchParams.get('id')||'';if(!/^[a-f0-9]{32}$/.test(id))return respond(400,{error:'Invalid report ID'});await store.delete(id)}
    return respond(200,{ok:true});
   }
   return respond(404,{error:'Not found'});
  }catch{return respond(503,{error:'Service unavailable. No diagnostic body was logged.'})}
 }
}
export default createHandler();
