import { createHmac } from 'node:crypto';
export class Store {
 constructor(env=process.env){this.url=env.UPSTASH_REDIS_REST_URL||env.KV_REST_API_URL;this.token=env.UPSTASH_REDIS_REST_TOKEN||env.KV_REST_API_TOKEN;this.secret=env.SESSION_SECRET;if(!this.url||!this.token||!this.secret)throw Error('Private storage not configured');if(new URL(this.url).protocol!=='https:')throw Error('HTTPS storage required')}
 async command(...args){let r=await fetch(this.url,{method:'POST',headers:{Authorization:`Bearer ${this.token}`,'Content-Type':'application/json'},body:JSON.stringify(args),signal:AbortSignal.timeout(8000)});if(!r.ok)throw Error('Storage unavailable');let data=await r.json();if(data.error)throw Error('Storage command failed');return data.result}
 async rateLimit(ip){let day=new Date().toISOString().slice(0,10),hash=createHmac('sha256',this.secret).update(day+'|'+ip).digest('hex');return Number(await this.command('EVAL',"local n=redis.call('INCR',KEYS[1]);if n==1 then redis.call('EXPIRE',KEYS[1],60) end;local g=redis.call('INCR',KEYS[2]);if g==1 then redis.call('EXPIRE',KEYS[2],3600) end;if n>30 or g>5000 then return 0 end;return 1",2,'limit:'+hash,'limit:global:'+Math.floor(Date.now()/3600000)))===1}
 // Code sign-in attempts: at most 5 per source and 30 overall per 15 minutes, so a short code cannot be guessed.
 async loginAttempt(ip){let hash=createHmac('sha256',this.secret).update('login|'+ip).digest('hex');return Number(await this.command('EVAL',"local n=redis.call('INCR',KEYS[1]);if n==1 then redis.call('EXPIRE',KEYS[1],900) end;local g=redis.call('INCR',KEYS[2]);if g==1 then redis.call('EXPIRE',KEYS[2],900) end;if n>5 or g>30 then return 0 end;return 1",2,'login:'+hash,'login:all'))===1}
 // Triage marks belong to an issue group (fingerprint), so they cover every report of the same problem.
 async marks(fingerprints){if(!fingerprints.length)return [];return (await this.command('MGET',...fingerprints.map(f=>'mark:'+f))).map(v=>v||null)}
 async mark(fingerprint,status){if(status==='open')await this.command('DEL','mark:'+fingerprint);else await this.command('SET','mark:'+fingerprint,status,'EX',2592000)}
 async save(report){
  const now=Date.now(),ttl=604800;
  if(report.kind!=='heartbeat'){
   const script="redis.call('SET',KEYS[1],ARGV[1],'EX',ARGV[2]);redis.call('ZADD',KEYS[2],ARGV[3],ARGV[4]);local old=redis.call('ZRANGE',KEYS[2],0,-1001);for _,id in ipairs(old) do redis.call('DEL','report:'..id);redis.call('ZREM',KEYS[2],id) end;redis.call('ZREMRANGEBYSCORE',KEYS[2],'-inf',ARGV[5]);redis.call('EXPIRE',KEYS[2],ARGV[2]);return 1";
   await this.command('EVAL',script,2,'report:'+report.id,'reports',JSON.stringify(report),ttl,now,report.id,now-ttl*1000);
  }
  const summary=JSON.stringify({sessionId:report.sessionId,lastSeen:now,scene:report.scene,uptimeSeconds:report.uptimeSeconds,serverUptimeSeconds:report.serverUptimeSeconds,release:report.release});
  await this.command('EVAL',"redis.call('SET',KEYS[1],ARGV[1],'EX',ARGV[2]);redis.call('ZADD',KEYS[2],ARGV[3],ARGV[4]);local old=redis.call('ZRANGE',KEYS[2],0,-501);for _,id in ipairs(old) do redis.call('DEL','session:'..id);redis.call('ZREM',KEYS[2],id) end;redis.call('ZREMRANGEBYSCORE',KEYS[2],'-inf',ARGV[5]);redis.call('EXPIRE',KEYS[2],ARGV[2]);return 1",2,'session:'+report.sessionId,'sessions',summary,ttl,now,report.sessionId,now-ttl*1000);
 }

 async list(){const ids=await this.command('ZREVRANGE','reports',0,99);if(!ids.length)return [];const raw=await this.command('MGET',...ids.map(id=>'report:'+id));return raw.filter(Boolean).map(v=>JSON.parse(v))}
 async sessions(){const ids=await this.command('ZRANGEBYSCORE','sessions',Date.now()-604800000,'+inf','LIMIT',0,500);if(!ids.length)return [];return (await this.command('MGET',...ids.map(id=>'session:'+id))).filter(Boolean).map(v=>JSON.parse(v))}
 async get(id){let data=await this.command('GET','report:'+id);return data?JSON.parse(data):null}
 async delete(id){await this.command('DEL','report:'+id);await this.command('ZREM','reports',id)}
 async purge(){for(let i=0;i<10;i++){const ids=await this.command('ZRANGE','reports',0,99);if(!ids.length)break;await this.command('DEL',...ids.map(id=>'report:'+id));await this.command('ZREM','reports',...ids)}const sessions=await this.command('ZRANGE','sessions',0,499);if(sessions.length)await this.command('DEL',...sessions.map(id=>'session:'+id));await this.command('DEL','sessions')}
}
