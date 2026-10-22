import fs from 'node:fs'
const {cookies, ua} = JSON.parse(fs.readFileSync('/tmp/site-cookies.json','utf8'))
const cl = cookies.find(c=>c.name==='cf_clearance').value
const base='https://www.touchgal.ink'
const H={'user-agent':ua,'accept':'*/*','x-requested-with':'kun-fetch','cookie':'cf_clearance='+cl,'origin':base,'referer':base+'/', 'content-type':'application/json'}
// gather pan links across recent patches
const r = await fetch(base+'/api/galgame?page=1&limit=24&sortField=resource_update_time&sortOrder=desc&selectedType=all&selectedLanguage=all&selectedPlatform=all&yearString=%5B%22all%22%5D&monthString=%5B%22all%22%5D&minRatingCount=0',{headers:H})
const list = await r.json()
const found = []
for (const g of list.galgames.slice(0, 10)) {
  const rr = await fetch(base+'/api/patch/resource?patchId='+g.id, {headers:H})
  const arr = await rr.json()
  for (const res of arr) for (const l of res.links) {
    if (l.content.includes('pan.touchgal.net')) found.push({patch:g.name, storage:l.storage, content:l.content, code:l.code, password:l.password})
  }
}
console.log(JSON.stringify(found.slice(0,10), null, 1))
// check each share info for locked
for (const f of found.slice(0,6)) {
  const key = f.content.split('/s/')[1]
  const i = await fetch('https://pan.touchgal.net/api/v3/share/info/'+key).then(r=>r.json()).catch(e=>({err:String(e)}))
  console.log(key, JSON.stringify(i).slice(0,200))
}
