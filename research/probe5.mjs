import fs from 'node:fs'
const {cookies, ua} = JSON.parse(fs.readFileSync('/tmp/site-cookies.json','utf8'))
const cl = cookies.find(c=>c.name==='cf_clearance').value
const base='https://www.touchgal.ink'
const H={'user-agent':ua,'accept':'*/*','x-requested-with':'kun-fetch','cookie':'cf_clearance='+cl,'origin':base,'referer':base+'/search','content-type':'application/json'}
async function s(label, extra) {
  const body={queryString:'[]',limit:5,searchOption:{searchInIntroduction:false,searchInAlias:true,searchInTag:false},page:1,selectedType:'all',selectedLanguage:'all',selectedPlatform:'all',sortField:'created',sortOrder:'desc',selectedYears:['all'],selectedMonths:['all'],minRatingCount:0,...extra}
  const r=await fetch(base+'/api/search',{method:'POST',headers:H,body:JSON.stringify(body)})
  const j=await r.json()
  console.log(label, r.status, j.total, (j.galgames||[]).map(g=>g.created.slice(0,7)).join(','))
}
await s('years2016',{selectedYears:['2016']})
await s('years2016,2017',{selectedYears:['2016','2017']})
await s('months7',{selectedMonths:['7']})
await s('future',{selectedYears:['future']})
await s('type game',{selectedType:'game'})
await s('lang ja',{selectedLanguage:'ja'})
await s('platform apk',{selectedPlatform:'apk'})
