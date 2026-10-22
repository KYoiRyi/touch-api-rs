import fs from 'node:fs'
const {cookies, ua} = JSON.parse(fs.readFileSync('/tmp/site-cookies.json','utf8'))
const cl = cookies.find(c=>c.name==='cf_clearance').value
const base = 'https://www.touchgal.ink'
const H = {'user-agent': ua, 'accept': '*/*', 'accept-language':'zh-CN,zh;q=0.9', 'x-requested-with':'kun-fetch', 'cookie':'cf_clearance='+cl, 'origin': base, 'referer': base+'/a555829b', 'content-type':'application/json'}

// 1. download stats (anonymous)
let r = await fetch(base+'/api/patch/resource/download', {method:'PUT', headers:H, body: JSON.stringify({patchId:637, resourceId:9094, linkId:8675})})
console.log('dl-stat anon:', r.status, (await r.text()).slice(0,200))

// 2. captcha challenge + solve
r = await fetch(base+'/api/auth/captcha/challenge', {method:'POST', headers:H})
const ch = await r.json()
console.log('challenge:', JSON.stringify(ch).slice(0,150))

// 3. solve in JS mirroring capjs-core
const fnv1a = (s) => { let h = 2166136261; for (let i=0;i<s.length;i++){ h ^= s.charCodeAt(i); h += (h<<1)+(h<<4)+(h<<7)+(h<<8)+(h<<24) } return h >>> 0 }
const fnv1aResume = (st, s) => { let h = st; for (let i=0;i<s.length;i++){ h ^= s.charCodeAt(i); h += (h<<1)+(h<<4)+(h<<7)+(h<<8)+(h<<24) } return h >>> 0 }
const prngFromHash = (init, len) => { let st = init, out=''; while (out.length < len){ st ^= st<<13; st ^= st>>>17; st ^= st<<5; st >>>= 0; out += st.toString(16).padStart(8,'0') } return out.substring(0,len) }
const { createHash } = await import('node:crypto')
const tf = fnv1a(ch.token)
const solutions = []
for (let i=0;i<ch.challenge.c;i++){
  const saltSeed = fnv1aResume(tf, String(i+1))
  const target = prngFromHash(fnv1aResume(saltSeed, 'd'), ch.challenge.d)
  const salt = prngFromHash(saltSeed, ch.challenge.s)
  let n = 0
  while (true) { const h = createHash('sha256').update(salt + n).digest('hex'); if (h.startsWith(target)) { solutions.push(n); break } n++ }
}
console.log('solved', solutions.length, solutions.slice(0,3))

r = await fetch(base+'/api/auth/captcha/redeem', {method:'POST', headers:H, body: JSON.stringify({token: ch.token, solutions})})
const red = await r.json()
console.log('redeem:', JSON.stringify(red).slice(0,200))

r = await fetch(base+'/api/auth/login', {method:'POST', headers:H, body: JSON.stringify({name: process.env.TG_USER || 'YOUR_USERNAME', password: process.env.TG_PASS || 'YOUR_PASSWORD', captcha: red.token})})
console.log('login:', r.status, (await r.text()).slice(0,300))
const setCookie = r.headers.getSetCookie ? r.headers.getSetCookie() : []
console.log('set-cookie:', JSON.stringify(setCookie).map(s=>s.split(';')[0]))
