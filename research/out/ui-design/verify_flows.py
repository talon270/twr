from pathlib import Path
import json
from playwright.sync_api import sync_playwright
ROOT=Path(__file__).resolve().parent
results=[]
with sync_playwright() as p:
 b=p.chromium.launch(executable_path='/usr/bin/chromium',args=['--no-sandbox'])
 for v in 'abc':
  page=b.new_page(viewport={'width':1500,'height':844},color_scheme='dark');errors=[]
  page.on('pageerror',lambda e:errors.append(str(e)))
  page.on('console',lambda m:errors.append(m.text) if m.type=='error' else None)
  page.goto((ROOT/(v+'.html')).as_uri());page.wait_for_timeout(200)
  checked=[]
  for cs,verb,suffix,phase in [('SEJ204','PUSH','TAIL W','Holding'),('IGO621','LINEUP','29R','Lined up'),('IGO621','CROSS','29R','Holding'),('AIC101','TAKEOFF','29R','Takeoff'),('UAE516','LAND','29L','Landing'),('IGO713','GOAROUND','','Go around'),('QTR570','EXIT','D','Holding')]:
   before=page.evaluate('JSON.stringify(mockup.state.traffic)')
   page.locator(f'.strip[data-cs="{cs}"]').click()
   page.locator(f'[data-verb="{verb}"]').click()
   page.locator('#command').fill(' '.join([cs,verb,suffix]).strip())
   assert page.locator('#send').is_enabled()
   page.locator('#send').click()
   assert page.evaluate(f'mockup.state.traffic.find(a=>a.cs==="{cs}").phase')==phase
   if verb in ['TAKEOFF','LAND']:assert page.locator('#movements').inner_text()=='25'
   page.locator('#undo').click()
   assert page.evaluate('JSON.stringify(mockup.state.traffic)')==before
   checked.append(verb)
  # Preview + applied route screenshot, with persistent undo.
  page.locator('.strip[data-cs="IGO512"]').click();page.locator('#destination').click();page.locator('#send').click()
  page.locator('#toast').hover();page.wait_for_timeout(230)
  page.screenshot(path=str(ROOT/'shots'/(v+'-1500-dark-route.png')))
  page.locator('#undo').click()
  # Reset has a real reversible state, rather than a dead demonstration button.
  page.locator('#command').fill('AIC304 HOLD');page.locator('#send').click();assert page.evaluate('mockup.state.activeConflict') is False
  page.locator('#results').click();page.locator('#reset-demo').click();page.wait_for_timeout(250)
  assert page.evaluate('mockup.state.activeConflict') is True
  page.locator('#undo').click();assert page.evaluate('mockup.state.activeConflict') is False
  checked.append('RESET_AND_UNDO')
  # Focus holds Undo beyond the original six-second deadline.
  page.locator('#command').fill('IGO512 HOLD');page.locator('#send').click();page.locator('#undo').focus();left=page.evaluate('mockup.state.undoLeft')
  page.mouse.move(5,5);page.wait_for_timeout(6200);assert abs(page.evaluate('mockup.state.undoLeft')-left)<110
  page.locator('#map').focus();page.wait_for_timeout(6400);assert not page.locator('#toast').evaluate('e=>e.classList.contains("shown")');assert page.locator('#toast').evaluate('e=>e.inert')
  checked.append('UNDO_FOCUS_PAUSE_AND_EXPIRY')
  # The advertised demo clock actually advances and can stop.
  old=page.locator('#clock').inner_text();page.locator('#pause').click();page.wait_for_timeout(1250);page.locator('#pause').click();new=page.locator('#clock').inner_text();assert old!=new
  page.wait_for_timeout(1100);assert page.locator('#clock').inner_text()==new
  checked.append('CLOCK_ADVANCE_AND_PAUSE')
  # Navigate to a sibling standalone file using its real footer link.
  nxt='b' if v=='a' else 'c' if v=='b' else 'a'
  page.locator(f'footer a[href="{nxt}.html"]').click();assert page.url.endswith(nxt+'.html');assert page.locator('.strip').count()==12
  checked.append('SIBLING_NAVIGATION')
  assert not errors,errors
  results.append({'direction':v,'checks':checked,'page_and_console_errors':errors})
  print('PASS',v,', '.join(checked),flush=True);page.close()
 b.close()
(ROOT/'flow-verification.json').write_text(json.dumps(results,indent=2))
print('3/3 extended flows passed; 0 page or console errors.')
