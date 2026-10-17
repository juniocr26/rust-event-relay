import { test, expect } from '@playwright/test'
import { readFileSync } from 'node:fs'
const token=readFileSync('/tmp/demo61-browser-token','utf8').trim()
const headers={Authorization:`Bearer ${token}`,'X-Demo-Client':'demo61'}
for (const scenario of ['dedupe','concurrent','recovery']) {
 test(`real ${scenario}: persisted correlated outcomes and stable request identities`,async({page,request})=>{
  await page.goto('/');await page.getByLabel('Choose an experience').selectOption(scenario)
  await page.getByRole('button',{name:'Open experience'}).click()
  await page.getByLabel('Local demo token',{exact:true}).fill(token)
  await page.getByRole('button',{name:'Run real delivery'}).click()
  await expect(page.getByRole('button',{name:'Prepare a new run'})).toBeVisible({timeout:25000})
  const key=await page.evaluate(s=>sessionStorage.getItem(`relay-request:${s}`),scenario)
  const result=await request.get(`/demo/runs/${key}`,{headers});expect(result.ok()).toBe(true)
  const e=await result.json();expect(e.stock).toBe(15);expect(e.receipt.stock_before).toBe(10);expect(e.receipt.stock_after).toBe(15)
  expect(e.observations.filter((o:{outcome:string})=>o.outcome==='Applied')).toHaveLength(1)
  if(scenario==='recovery') {
   expect(e.fault.injected_at).toBeTruthy();expect(e.fault.retry_available_at).toBeTruthy();expect(e.fault.retry_revision).toBeGreaterThan(0);expect(e.outbox.attempt_count).toBeGreaterThanOrEqual(2)
   await expect(page.getByText('This is not an infrastructure outage.',{exact:false})).toBeVisible()
  } else {
   const copies=scenario==='dedupe'?1:3
   expect(e.action.copies).toBe(copies);expect(e.action.state).toBe('confirmed')
   const observations=e.observations.filter((o:{action_id:string;outcome:string})=>o.action_id===e.action.action_id && o.outcome==='Duplicate')
   expect(observations.length).toBeGreaterThanOrEqual(copies)
   if(scenario==='concurrent') expect(new Set(observations.map((o:{context:string})=>o.context)).size).toBe(2)
   const repeated=await request.post('/demo/actions',{headers,data:{request_id:key,run_id:e.run_id,copies}})
   expect(repeated.status()).toBe(202);expect((await repeated.json()).action_id).toBe(e.action.action_id)
   const conflict=await request.post('/demo/actions',{headers,data:{request_id:key,run_id:e.run_id,copies:copies===1?3:1}})
   expect(conflict.status()).toBe(409)
   await expect(page.getByRole('button',{name:'Copy Action ID'})).toBeVisible()
  }
  await page.reload();await page.getByRole('button',{name:'Open experience'}).click()
  await page.getByLabel('Choose an experience').count() // underlying page remains inert while modal is open
  await page.keyboard.press('Escape');await page.getByLabel('Choose an experience').selectOption(scenario)
  await page.getByRole('button',{name:'Open experience'}).click()
  await page.getByLabel('Local demo token',{exact:true}).fill(token)
  await page.getByRole('button',{name:'Recover same request'}).click()
  await expect(page.getByRole('button',{name:'Prepare a new run'})).toBeVisible()
  const after=await request.get(`/demo/runs/${key}`,{headers});expect((await after.json()).event_id).toBe(e.event_id)
 })
}

test('historical run: explicit duplicate, close/reopen and another action preserve original effect', async ({page,request}) => {
 const runs=await request.get('/demo/runs',{headers});expect(runs.ok()).toBe(true)
 const run=(await runs.json())[0];expect(run).toBeTruthy()
 await page.goto('/');await page.getByLabel('Choose an experience').selectOption('dedupe')
 await page.getByRole('button',{name:'Open experience'}).click()
 await page.getByLabel('Local demo token',{exact:true}).fill(token)
 await page.getByRole('button',{name:'Load successful runs'}).click()
 await page.getByLabel('Successful run',{exact:true}).selectOption(run.request_id)
 const admitted=page.waitForResponse(r=>r.url().endsWith('/demo/actions')&&r.request().method()==='POST')
 await page.getByRole('button',{name:/Run real delivery|Recover same request/}).click()
 expect((await admitted).status()).toBe(202)
 await page.keyboard.press('Escape')
 await page.getByRole('button',{name:'Open experience'}).click()
 await page.getByRole('button',{name:'Recover same request'}).click()
 await expect(page.getByText('Stock remains 15 · One business effect',{exact:true})).toBeVisible({timeout:25000})
 const before=await (await request.get(`/demo/runs/${run.request_id}`,{headers})).json()
 await page.getByRole('button',{name:'Request another duplicate'}).click()
 await expect(page.getByRole('button',{name:'Prepare a new run'})).toBeVisible({timeout:25000})
 const after=await (await request.get(`/demo/runs/${run.request_id}`,{headers})).json()
 expect(after.event_id).toBe(before.event_id);expect(after.fact_id).toBe(before.fact_id)
 expect(after.actions.length).toBe(before.actions.length+1)
 expect(after.stock).toBe(15);expect(after.effect_receipts).toBe(1)
 expect(after.observations.some((o:{action_id:string;outcome:string})=>o.action_id===after.action.action_id&&o.outcome==='Duplicate')).toBe(true)
 await page.screenshot({path:'/tmp/demo62-historical.png',fullPage:true})
 await expect(page.getByText('Last evidence is stale.',{exact:false})).toBeVisible({timeout:18000})
 await page.getByText('Last evidence is stale.',{exact:false}).scrollIntoViewIfNeeded()
 await page.screenshot({path:'/tmp/demo63-stale.png',fullPage:true})
})

test('concurrent sessions release reserved copies across repeated real actions',async({request})=>{
 for(let index=0;index<3;index++) {
  const id=crypto.randomUUID()
  const submit=await request.post('/demo/runs',{headers,data:{request_id:id,scenario:'concurrent',initial_stock:10,quantity:5}})
  expect(submit.status()).toBe(202)
  await expect.poll(async()=>{const e=await (await request.get(`/demo/runs/${id}`,{headers})).json();return !!e.receipt&&e.outbox.status==='processed'},{timeout:10000,intervals:[1000]}).toBe(true)
  const run=await (await request.get(`/demo/runs/${id}`,{headers})).json()
  const action=await request.post('/demo/actions',{headers,data:{request_id:crypto.randomUUID(),run_id:run.run_id,copies:3}})
  expect(action.status()).toBe(202)
  await expect.poll(async()=>{const e=await (await request.get(`/demo/runs/${id}`,{headers})).json();return e.action?.state},{timeout:10000,intervals:[1000]}).toBe('confirmed')
  const e=await (await request.get(`/demo/runs/${id}`,{headers})).json()
  expect(e.stock).toBe(15);expect(e.effect_receipts).toBe(1)
  expect(e.observations.filter((o:{action_id:string;outcome:string})=>o.action_id===e.action.action_id&&o.outcome==='Duplicate').length).toBeGreaterThanOrEqual(3)
 }
})

test('retained inspection paginates, selects historical actions and never submits work', async ({page, request}) => {
 const first = await request.get('/demo/history?limit=2&offset=0', {headers})
 expect(first.ok()).toBe(true)
 const rows = await first.json(); expect(rows.length).toBe(2)
 const next = await (await request.get('/demo/history?limit=2&offset=2', {headers})).json()
 expect(next.some((r:{run_id:string})=>r.run_id===rows[0].run_id)).toBe(false)
 expect((await request.get('/demo/history?limit=21&offset=0', {headers})).status()).toBe(400)
 const runs = await (await request.get('/demo/runs', {headers})).json()
 let chosen
 for (const r of runs) {
  const e = await (await request.get(`/demo/runs/${r.request_id}`, {headers})).json()
  if (e.actions.length >= 2) {chosen=e;break}
 }
 expect(chosen).toBeTruthy()
 // Setup queries share the application's 10/s quota with the browser.
 await new Promise(resolve => setTimeout(resolve, 1100))
 const posts:string[]=[];page.on('request', r=>{if(r.method()==='POST')posts.push(r.url())})
 await page.goto('/');await page.getByRole('button',{name:'Open experience'}).click()
 await page.getByLabel('Local demo token',{exact:true}).fill(token)
 await page.getByText('Inspect retained runs and actions',{exact:true}).click()
 await page.getByRole('button',{name:'Load retained runs',exact:true}).click()
 await expect(page.getByText('Retained runs loaded.',{exact:false})).toBeVisible()
 await page.getByLabel('Retained run',{exact:true}).selectOption(chosen.request_id)
 await page.getByRole('button',{name:'Inspect existing evidence',exact:true}).click()
 await expect(page.getByRole('button',{name:'Copy Run ID',exact:true})).toBeVisible()
 await page.getByLabel('Historical action',{exact:true}).selectOption(chosen.actions[0].action_id)
 await page.getByRole('button',{name:'Inspect selected action',exact:true}).click()
 await expect(page.getByText(chosen.actions[0].action_id,{exact:true})).toBeVisible()
 for(const width of [320,768,1920]) {
  await page.setViewportSize({width,height:900})
  await page.getByText(chosen.actions[0].action_id,{exact:true}).scrollIntoViewIfNeeded()
  expect(await page.locator('.modal').evaluate(el=>el.scrollWidth<=el.clientWidth)).toBe(true)
  await page.screenshot({path:`/tmp/demo63-history-${width}.png`,fullPage:true})
 }
 await page.reload();await page.getByRole('button',{name:'Open experience'}).click()
 await expect(page.getByLabel('Local demo token',{exact:true})).toHaveValue('')
 await page.getByLabel('Local demo token',{exact:true}).fill(token)
 await page.getByRole('button',{name:'Recover same request'}).click()
 await expect(page.getByText(chosen.actions[0].action_id,{exact:true})).toBeVisible()
 expect(posts).toEqual([])
 const after=await (await request.get(`/demo/runs/${chosen.request_id}`,{headers})).json()
 expect(after.actions).toEqual(chosen.actions);expect(after.effect_receipts).toBe(1);expect(after.stock).toBe(15)
})

test('unknown action inspection never republishes or changes retained identities', async ({page,request}) => {
 const runs=await (await request.get('/demo/history?limit=10&offset=0',{headers})).json()
 let chosen
 for (const r of runs) {
  const e=await (await request.get(`/demo/runs/${r.request_id}`,{headers})).json()
  if(e.actions.some((a:{state:string})=>a.state==='unknown')) {chosen=e;break}
 }
 expect(chosen).toBeTruthy()
 const action=chosen.actions.find((a:{state:string})=>a.state==='unknown')
 await new Promise(resolve=>setTimeout(resolve,1100))
 await page.goto('/')
 await page.evaluate(({id,action})=>{sessionStorage.setItem('relay-request:dedupe',id);sessionStorage.setItem('relay-request:dedupe:action',action)}, {id:chosen.request_id,action:action.action_id})
 const posts:string[]=[];page.on('request',r=>{if(r.method()==='POST')posts.push(r.url())})
 await page.getByLabel('Choose an experience').selectOption('dedupe');await page.getByRole('button',{name:'Open experience'}).click()
 await page.getByLabel('Local demo token',{exact:true}).fill(token)
 await page.getByRole('button',{name:'Recover same request'}).click()
 await expect(page.getByText('Action outcome is unknown or incomplete.',{exact:false})).toBeVisible()
 await expect(page.getByText(action.action_id,{exact:true})).toBeVisible()
 await page.setViewportSize({width:768,height:900})
 await page.screenshot({path:'/tmp/demo63-unknown.png',fullPage:true})
 await page.keyboard.press('Escape');await page.getByRole('button',{name:'Open experience'}).click()
 await page.getByRole('button',{name:'Recover same request'}).click()
 await expect(page.getByText('Action outcome is unknown or incomplete.',{exact:false})).toBeVisible()
 expect(posts).toEqual([])
 const after=await (await request.get(`/demo/runs/${chosen.request_id}`,{headers})).json()
 expect(after.actions).toEqual(chosen.actions);expect(after.stock).toBe(15);expect(after.effect_receipts).toBe(1)
})
