from playwright.sync_api import sync_playwright
from pathlib import Path
root=Path(__file__).resolve().parent
with sync_playwright() as p:
 b=p.chromium.launch(executable_path='/usr/bin/chromium',headless=True,args=['--no-sandbox'])
 for v in 'abc':
  page=b.new_page(viewport={'width':1500,'height':844},color_scheme='dark')
  errors=[];page.on('pageerror',lambda e:errors.append(str(e)))
  page.goto((root/(v+'.html')).as_uri());page.wait_for_timeout(250)
  page.screenshot(path=str(root/'shots'/('draft-'+v+'.png')))
  print(v,errors,page.evaluate('({body:document.body.scrollHeight,viewport:innerHeight,stage:document.querySelector(".map-stage").getBoundingClientRect().toJSON(),command:document.querySelector(".command").getBoundingClientRect().toJSON()})'))
  page.close()
 b.close()
