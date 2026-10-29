import { z } from 'zod';
export const scenes=['dashboard','library','collections','watchlist','settings','search','player','unknown','server'];
export const codes=['read-null','read-undefined','not-callable','not-defined','syntax','network','resource','unknown','http5xx','media-decode','media-network','hls-fatal','panic','heartbeat','test'];
export const kinds=['browser_error','rejection','http_error','playback_error','server_error','panic','heartbeat','test'];
const integer=(max=10000000)=>z.number().int().min(0).max(max);
const box=z.object({x:integer(20000),y:integer(20000),w:integer(20000),h:integer(20000),kind:z.enum(['button','input','select','panel','media','link','unknown'])}).strict();
const step=z.object({kind:z.enum(['snapshot','click','navigation','scroll','resize','input','error']),offsetMs:integer(86400000),scene:z.enum(scenes),target:box.shape.kind.optional(),x:integer(20000).optional(),y:integer(20000).optional(),width:integer(20000).optional(),height:integer(20000).optional(),boxes:z.array(box).max(60).optional()}).strict();
const player=z.object({active:z.boolean(),mode:z.enum(['direct','hls','progressive','unknown']),quality:z.enum(['auto','1080','720','480','unknown']),timeBucket:integer(10000),durationBucket:integer(10000),paused:z.boolean(),readyState:integer(4),error:integer(4)}).strict();
export const reportSchema=z.object({
 schema:z.literal(1),sessionId:z.string().regex(/^[a-f0-9]{32}$/),kind:z.enum(kinds),code:z.enum(codes),scene:z.enum(scenes),
 uptimeSeconds:integer(31536000),serverUptimeSeconds:integer(31536000).optional(),status:integer(599).optional(),
 route:z.enum(['playback','subtitles','metadata','library','auth','settings','scan','other']).optional(),
 errorType:z.enum(['Error','TypeError','ReferenceError','SyntaxError','RangeError','DOMException','Other']).optional(),
 frames:z.array(z.object({asset:z.enum(['app','hls','telemetry','server']),line:integer(),column:integer()}).strict()).max(12).default([]),
 context:z.object({browser:z.enum(['Chrome','Firefox','Safari','Edge','Other']).optional(),browserMajor:integer(1000).optional(),platform:z.enum(['Windows','macOS','Linux','Android','iOS','Other']).optional(),width:integer(20000).optional(),height:integer(20000).optional(),pixelRatio:z.number().min(0).max(10).optional(),cores:integer(256).optional(),online:z.boolean().optional(),mse:z.boolean().optional(),wasm:z.boolean().optional(),fullscreen:z.boolean().optional(),pip:z.boolean().optional(),player:player.optional(),runtime:z.enum(['browser','rust']).optional(),os:z.enum(['linux','macos','windows','other']).optional(),arch:z.enum(['x86_64','aarch64','other']).optional()}).strict(),
 replay:z.array(step).max(100).default([]),
 release:z.string().regex(/^[0-9]+\.[0-9]+\.[0-9]+(?:-[a-z0-9.]+)?$/).max(40),
}).strict();
export function parseReport(input){return reportSchema.parse(input)}
