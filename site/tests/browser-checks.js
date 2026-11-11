async(page)=>{
const checks=[],errors=[];page.on('pageerror',e=>errors.push(e.message));
function ok(name,value){if(!value)throw Error(name);checks.push(name);}
await page.setViewportSize({width:1440,height:1000});await page.goto('http://127.0.0.1:3011/demo/');
await page.getByLabel('Dynamic Island simulée',{exact:true}).focus();await page.keyboard.press('Enter');await page.waitForTimeout(300);
ok('dimensions développées',Math.round((await page.locator('.island').boundingBox()).width)===384);
await page.getByRole('button',{name:'Lire le média simulé',exact:true}).click();await page.waitForTimeout(2200);
ok('progression média',!(await page.locator('.media-time').innerText()).startsWith('00:00'));
await page.getByRole('button',{name:'Piste suivante',exact:true}).click();ok('piste suivante',(await page.locator('.track-info strong').innerText())==='Les heures bleues');
await page.getByRole('button',{name:'Concentration',exact:true}).click();ok('ligne concentration',Math.round((await page.locator('.island').boundingBox()).height)===4);
await page.locator('.desktop-window').click();ok('clic traversant',(await page.locator('.note-content em').innerText()).includes('1 clic'));
await page.getByRole('button',{name:'Plein écran fictif',exact:true}).click();await page.getByRole('button',{name:'Normal',exact:true}).click();ok('priorité plein écran',await page.locator('.island').isHidden());
await page.getByRole('button',{name:'Plein écran fictif',exact:true}).click();await page.getByRole('button',{name:'Ouvrir les minuteurs',exact:true}).click();await page.getByRole('button',{name:'Démarrer',exact:true}).click();await page.waitForTimeout(1200);
ok('minuteur avance',(await page.locator('.native-timer>strong').innerText())!=='25:00');
await page.getByRole('button',{name:'Pause',exact:true}).click();const paused=await page.locator('.native-timer>strong').innerText();await page.waitForTimeout(1100);ok('pause stable',paused===await page.locator('.native-timer>strong').innerText());
await page.keyboard.press('Escape');await page.waitForTimeout(250);ok('Échap replie',Math.round((await page.locator('.island').boundingBox()).width)===172);
await page.getByRole('button',{name:'Ouvrir les raccourcis',exact:true}).click();await page.getByRole('button',{name:'Gérer…',exact:true}).click();await page.getByLabel('Nom',{exact:true}).fill('Essai local');await page.getByLabel('Cible fictive',{exact:true}).fill('Projet fictif');await page.getByRole('button',{name:'Ajouter',exact:true}).click();ok('ajout catalogue',await page.getByRole('button',{name:'Supprimer Essai local',exact:true}).isVisible());
await page.getByRole('button',{name:'Favori Essai local',exact:true}).click();await page.getByRole('button',{name:'Terminer',exact:true}).click();await page.getByRole('button',{name:'☆ Tout',exact:true}).click();ok('filtre favori',(await page.locator('.native-launcher').innerText()).includes('Essai local'));
await page.keyboard.press('Escape');await page.getByRole('button',{name:'Ouvrir les raccourcis',exact:true}).click();await page.getByRole('button',{name:'Gérer…',exact:true}).click();ok('catalogue conservé au repli',await page.getByRole('button',{name:'Supprimer Essai local',exact:true}).isVisible());await page.getByRole('button',{name:'Supprimer Essai local',exact:true}).click();await page.getByRole('button',{name:'Terminer',exact:true}).click();
await page.getByRole('button',{name:'Concentration',exact:true}).click();ok('code synchronisé',(await page.locator('.code-description h3').innerText()).includes('concentration'));
await page.getByRole('button',{name:'Normal',exact:true}).click();await page.getByRole('button',{name:'Ouvrir les médias',exact:true}).click();
await page.getByLabel('Session multimédia fictive').selectOption('none');ok('session absente',await page.locator('.empty-media').isVisible());
await page.getByLabel('Session multimédia fictive').selectOption('studio');
await page.locator('summary').click();await page.getByLabel('État du média').selectOption('limited');ok('capacité désactivée',await page.getByRole('button',{name:'Piste suivante',exact:true}).isDisabled());
await page.emulateMedia({reducedMotion:'reduce'});await page.keyboard.press('Escape');await page.getByLabel('Dynamic Island simulée',{exact:true}).focus();await page.keyboard.press('Enter');await page.waitForTimeout(60);ok('mouvement réduit',Math.round((await page.locator('.island').boundingBox()).width)===384);
for(const width of [360,768,1440]){await page.setViewportSize({width,height:900});await page.waitForTimeout(100);const geometry=await page.evaluate(()=>{const s=document.querySelector('.desktop').getBoundingClientRect(),i=document.querySelector('.island').getBoundingClientRect();return {ok:document.documentElement.scrollWidth<=innerWidth&&i.left>=s.left-1&&i.right<=s.right+1};});ok('responsive '+width,geometry.ok);}
for(const route of ['code','histoire','sous-le-capot']){const r=await page.goto('http://127.0.0.1:3011/'+route+'/');ok('route '+route,r.status()===200);}
await page.getByRole('button',{name:/island-core/}).click();ok('graphe interactif',(await page.locator('.architecture-detail h3').innerText())==='Les règles du jeu');
ok('aucune erreur JavaScript',errors.length===0);
return {checks,errors};
}
