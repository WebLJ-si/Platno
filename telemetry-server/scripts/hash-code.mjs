// Prints the DASHBOARD_CODE_HASH value for an access code. The code is read from the first argument or,
// better, from standard input so it stays out of the shell history:   printf '%s' 2828 | node scripts/hash-code.mjs
import { hashCode } from '../lib/auth.mjs';
let code=process.argv[2];
if(code===undefined){code='';for await(const chunk of process.stdin)code+=chunk}
code=code.replace(/\r?\n$/,'');
if(!code){console.error('Give a code as an argument or on standard input.');process.exit(2)}
console.log(hashCode(code));
