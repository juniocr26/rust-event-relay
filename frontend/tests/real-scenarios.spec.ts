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
