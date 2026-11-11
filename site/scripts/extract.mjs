import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const base=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
// Explicit, reviewable allowlist. No preferences, logs, whole-repository export or runtime file API.
const entries=[
 ['transition','island-core/src/lib.rs','    pub fn sample(', '    pub fn active(', 'Ouvrir et replier','Une interpolation smoothstep, 220 ms à l’ouverture et 180 ms à la fermeture. Une inversion repart de la position courante.'],
 ['concentration','island-windows/src/concentration.rs','    pub(super) fn set_concentration(', '\n}\n', 'Passer en concentration','Le panneau se replie immédiatement, libère le geste et le focus, puis devient traversant aux clics. Dans cette démo, seuls les éléments du bureau virtuel reçoivent ces clics.'],
 ['visibility','island-core/src/visibility.rs','    pub fn hidden(', '\n}\n','Masquer et restaurer','Les causes de masquage sont indépendantes du mode visuel. Le plein écran reste prioritaire sur une restauration manuelle.'],
 ['placement','island-core/src/placement.rs','    pub fn update(', '\n}\n','Déplacer l’encoche','Le geste doit dépasser un seuil. Le relâchement valide la position ; Échap annule. Le navigateur conserve le déplacement dans les limites de la scène.'],
 ['media','island-windows/src/media.rs','    pub fn command(', '\n}\n','Contrôler un média','Le worker reçoit une commande bornée avec identité et génération. Ici, les commandes agissent exclusivement sur des sessions fictives et silencieuses.'],
 ['selection','island-core/src/media.rs','pub fn select(', 'pub fn bounded_text(', 'Choisir une session','La sélection native suit les sessions réellement exposées par GSMTC. Un lecteur absent de Windows ne peut pas être inventé par l’application.'],
 ['modules','island-core/src/modules.rs','    pub fn navigate(', '    pub fn media_visible(', 'Changer de module','La navigation refuse une page désactivée ; la désactivation de la page active revient à l’accueil.'],
 ['timer','island-core/src/timer.rs','    pub fn remaining(', '    pub fn primary_label(', 'Piloter le temps','Une échéance monotone remplace le comptage des ticks. Chaque nouvelle phase Pomodoro attend un démarrage explicite.'],
 ['launcher','island-core/src/launcher.rs','    pub fn upsert(', '    pub fn text(', 'Organiser ses raccourcis','Le catalogue natif est borné et validé. Les favoris présentés ici sont fictifs : aucun programme de votre ordinateur n’est lancé.'],
 ['render','island-ui/src/lib.rs','pub fn dimensions(', '#[derive(Clone, Copy, Debug)]','Dessiner la forme','La largeur, la hauteur et le rayon suivent la même progression. Compact : 172 × 32 DIP ; déplié : 384 × 168 DIP.'],
];
const result=[];
for(const [id,file,start,end,title,description] of entries){
 const source=(await readFile(path.join(base,'windows','crates',file),'utf8')).replace(/\r\n/g,'\n');
 const a=source.indexOf(start),b=source.indexOf(end,a+start.length);
 if(a<0||b<0||source.indexOf(start,a+start.length)>=0)throw Error(`Missing or ambiguous symbol: ${id}`);
 const code=source.slice(a,b).trimEnd();
 result.push({id,file:'windows/crates/'+file,line:source.slice(0,a).split('\n').length,code,title,description,sha256:createHash('sha256').update(source).digest('hex')});
}
await mkdir(path.join(base,'site/generated'),{recursive:true});
await writeFile(path.join(base,'site/generated/source.json'),JSON.stringify(result,null,2)+'\n');
console.log(`Extracted ${result.length} verified source excerpts.`);

