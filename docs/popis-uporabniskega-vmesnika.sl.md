# PlexV2 – podroben popis uporabniškega vmesnika

Ta dokument opisuje trenutno implementiran vmesnik na podlagi kode aplikacije. Imena kontrol so navedena tako, kot se prikažejo v angleškem vmesniku; razlaga je v slovenščini. Vsebina in razpoložljivi gumbi so odvisni od vloge, profila, izbranega medija in podatkov v knjižnici. Opis namenoma ne določa vizualnega oblikovanja vmesnika.

## 1. Prvi zagon: ustvarjanje lastniškega računa

Če strežnik še nima računa, se odpre stran **Create your server owner account**:

- **Email**: e-poštni naslov lastnika strežnika.
- **Display name**: neobvezno prikazno ime.
- **Password**: geslo lastnika; zahtevanih je vsaj 12 znakov.
- **Confirm password**: ponovni vnos gesla.
- **Create owner account**: ustvari račun in začetni profil. Ob neuspehu se prikaže sporočilo, na primer o neujemajočih se geslih ali neveljavnem naslovu.

## 2. Prijava in odjava

Ko je lastnik že ustvarjen, stran **Sign in to your media server** vsebuje:

- **Email** in **Password** za prijavo.
- **Sign in**, ki preveri prijavne podatke in odpre aplikacijo.
- Sporočilo ob napačni prijavi ali začasni omejitvi po več napačnih poskusih.
- Navodilo za obnovitev pozabljenega lastniškega gesla z ukazom v terminalu strežnika. Obnovitve gesla ni mogoče opraviti neposredno s klikom v tem obrazcu.

Po prijavi se uporabnik odjavi z **Sign out** v meniju profila. Odpre se prijavna stran.

## 3. Skupni okvir aplikacije

### Stranski meni

- **Overview**: domači pregled knjižnice in ogledov.
- **Libraries**: razpre ali zapre podmeni knjižnic. Pod njim so posamezne knjižnice, na primer **Movies**, **TV Shows** in **Videos**, ter število njihovih datotek. Imena so odvisna od nastavitev strežnika. Klik imena odpre izbrano knjižnico.
- **Watchlist**: seznam naslovov, shranjenih v aktivnem profilu.
- **Collections**: lokalne skupine oštevilčenih datotek in druge lokalne mape.
- **Settings**: uporabniške nastavitve; pri administratorju in lastniku tudi upravljanje strežnika.

V stranskem delu sta tudi ime strežnika oziroma aplikacije in pojasnilo, da mediji ostanejo na svojem mestu ter jih pregledovalnik bere.

### Glava

- Naslov pove, kateri pogled je trenutno odprt: na primer **Overview**, ime knjižnice, **Watchlist**, **Collections** ali **Settings**. Pod njim je kratek opis izbranega pogleda.
- **Search your library…** je globalno iskalno polje. Vnašanje odpre rezultate po naslovih in datotekah. Izbris iskalnega niza vrne prejšnji pogled. **Ctrl+K** oziroma **Cmd+K** postavi fokus v iskalno polje.
- Gumb **Profile menu** odpre meni aktivnega profila.

### Meni profila

Prikazuje ime aktivnega profila in e-poštni naslov računa:

- **Switch profile** in gumbi z imeni drugih profilov: izbira profila. Pred preklopom se zahteva PIN izbranega profila.
- Če drugih profilov ni, se prikaže **No other profiles yet.**
- **＋ Add profile**: odpre obrazec za nov profil.
- **Sign out**: odjavi trenutni račun v trenutni napravi.

### Podnožje

- **Help & documentation**: odpre vgrajena navodila.
- Navedena sta ponudnika metapodatkov TVmaze in TMDB ter povezavi do njunih strani.

### Obvestilo o skeniranju

Ko poteka skeniranje, se v vrhnjem delu prikaže obvestilo za administratorja oziroma lastnika. Vsebuje fazo, število najdenih oziroma obdelanih datotek, pretečeni čas, trenutno dejavnost, trenutno datoteko in nedavne dogodke. Med odkrivanjem datotek prikazuje število najdenih datotek; med obdelavo tudi napredek. Ustavitev se sproži z **Cancel scan** v nastavitvah.

## 4. Dodajanje in preklop profila

### Add a profile

- **Profile name**: ime novega profila.
- **PIN** in **Confirm PIN**: vnos in potrditev številskega PIN-a s štirimi do osmimi števkami.
- **Create profile**: ustvari profil; napake se prikažejo v obrazcu.
- **Cancel**: zapre obrazec brez ustvarjanja.

### Preklop profila

- Okno **Enter profile PIN** zahteva PIN izbranega profila.
- Če gre za starejši profil brez nastavljenega PIN-a, okno zahteva nastavitev in potrditev PIN-a.
- **Continue** preveri PIN in preklopi profil.
- **Cancel** zapre okno.
- Po več napačnih poskusih se lahko izpiše začasna omejitev poskusov.

Zgodovina, seznam za ogled in uporabniške nastavitve pripadajo aktivnemu profilu.

## 5. Overview

### Featured from your library

V ospredju je izbran naslov iz knjižnice, njegovo ime in osnovni podatki:

- **▶ Start watching** začne predvajati pripadajočo datoteko.
- **Explore library** odpre podrobnosti prikazanega naslova.
- Če knjižnica nima primernega naslova, se namesto tega prikaže **＋ Add your library**, ki odpre nastavitve.

### Library at a glance

Prikazani so števci:

- **Movies**: prepoznani filmi.
- **TV shows**: prepoznane serije.
- **Episodes**: prepoznane epizodne datoteke.
- **Watched**: število ogledanih datotek v aktivnem profilu.

### Police na domačem pregledu

- **Continue watching**: nedokončani filmi in epizode. Kartica pokaže naslov, položaj oziroma podatke o epizodi in napredek; klik odpre predvajanje. Če ni vsebine, je prikazano pojasnilo.
- **You might like**: lokalna priporočila glede na zgodovino aktivnega profila. Kartica odpre podrobnosti naslova. Če ni dovolj podatkov, se prikaže prazno stanje.
- **Popular in your library**: naslovi, razvrščeni glede na zgodovino ogledov in razpoložljivo knjižnično vsebino. Kartica odpre podrobnosti.
- **Explore your shows**: kartice serij in gumb **All shows →** za knjižnico serij.
- **Movie collections**: kartice filmov oziroma filmskih zbirk in gumb **All movies →** za filmsko knjižnico.

Prve tri police je mogoče posamezno vključiti ali izključiti v nastavitvah profila.

## 6. Knjižnice

Uporabnik izbere knjižnico v podmeniju **Libraries**. V glavi pogleda sta ime in tip knjižnice, v vsebini pa število datotek. Administrator oziroma lastnik vidi še **Manage libraries**, ki odpre nastavitve.

### Filmi in serije: filtri in razvrščanje

- **Year**: filtrira po letu.
- **Genre**: filtrira po žanru.
- **Language**: filtrira po jeziku metapodatkov.
- **Minimum rating**: zahteva vsaj izbrano oceno.
- **Time available**: izbere naslove, katerih znano trajanje ustreza razpoložljivemu času.
- **Sort**: **Title**, **Recently added**, **Year**, **Rating** ali **Shuffle**.
- **Unwatched**: omeji prikaz na naslove z vsaj eno neogledano datoteko.
- **Surprise me**: odpre naključno izbran naslov iz trenutno filtriranih rezultatov.
- Število zadetkov pove, koliko naslovov ustreza filtrom. Neznani podatki ne zadovoljijo filtra, ki jih zahteva.
- Kartica naslova vsebuje naslov, leto, če je znano, in število datotek; klik odpre podrobnosti. Kartico je mogoče odpreti tudi s tipkovnico.
- Če nič ne ustreza filtrom, se prikaže obvestilo, da ni ustreznih naslovov.

### Videos in druge knjižnice

Namesto filmskih oziroma serijskih metapodatkov se prikažejo lokalne mape. Kartica odpre podrobnosti mape s seznamom datotek. Posamezna datoteka ponuja **Edit identification** in **▶ Play**.

## 7. Iskanje

Globalno iskanje po naslovih in imenih oziroma poteh datotek prikaže:

- Število ustreznih datotek.
- **Movies**: kartice ujemajočih se filmov, če obstajajo.
- **TV shows**: kartice ujemajočih se serij, če obstajajo.
- **Files**: seznam ujemajočih se datotek z naslovom in potjo.
- Pri vsaki datoteki sta **Edit identification** in **▶ Play**.
- Če ni zadetkov, se pokaže **No matching files**. Pri več kot 200 datotekah se prikaže prvih 200 in navodilo za natančnejše iskanje.

## 8. Watchlist

Pogled **Your watchlist** kaže aktivni profil in število shranjenih postavk. Vsaka postavka ima naslov in razpoložljive podatke o letu oziroma sezoni in epizodi:

- **▶ Play**: predvajanje ustrezne datoteke; če datoteke v trenutnem seznamu ni, je gumb onemogočen.
- **Remove**: odstrani postavko s seznama, ne izbriše datoteke.

Prazni seznam pojasni, da je mogoče naslov dodati iz njegovih podrobnosti.

## 9. Collections in lokalne mape

**Collections** pojasni, da so lokalne zbirke sklepane iz mape in oštevilčenih datotek ter niso potrjen katalog. Prikazane so kartice lokalnih zbirk. Če obstajajo druge datoteke brez zanesljive identifikacije, se prikaže še **Other local folders**.

### Podrobnosti lokalne zbirke

- Naslov zbirke, pot mape in pojasnilo o lokalnem vrstnem redu.
- Seznam oštevilčenih datotek; ponovljena številka je označena z **Repeated number**.
- Pri posamezni datoteki so naslov, pot, dosegljivost in informacija, ali je ujemanje metapodatkov potrjeno.
- **Edit identification**: popravi prepoznavo datoteke.
- **▶ Play**: predvaja izbrano datoteko.
- **Close**: zapre podrobnosti.

### Podrobnosti lokalne mape

Prikazani so naslov, pot, opozorilo, da identiteta ni preverjena, ter seznam datotek. Vsaka ima **Edit identification** in **▶ Play**; **Close** zapre okno.

Kadar je v pogledu prikazana zbirka na podlagi TMDB, njene podrobnosti vsebujejo naslov, število filmov, kartice filmov in **Close**.

## 10. Podrobnosti filma ali serije

Okno naslova prikaže naslov, tip in ponudnika, leto, oceno, trajanje in opis, če so podatki na voljo. Med nalaganjem oziroma ob manjkajočih podatkih se prikaže ustrezno besedilo. **Close** zapre okno.

### Skupna dejanja

- **✓ Mark movie watched** oziroma **✓ Mark series watched**: označi pripadajoče datoteke kot ogledane.
- **Mark unwatched / reset**: počisti oznako ogledanosti in shranjeni položaj za pripadajoče datoteke.
- **＋ Add to Watchlist**: doda naslov na seznam aktivnega profila. Če je že dodan, je prikazano **✓ In Watchlist**; ponoven klik ga odstrani.
- Pod dejanji se izpiše potrditev označevanja oziroma ponastavitve.

### Film

- **▶ Play movie**: predvaja izbrano primarno datoteko filma.

### Serija

- **Season**: izbere sezono.
- **Episode**: izbere epizodo iz sezone; manjkajoča epizoda je označena kot **Missing**.
- **▶ Play episode**: predvaja izbrano epizodo, kadar je zanjo na voljo lokalna datoteka. Sicer je gumb onemogočen.
- **Mark selected season watched**: označi datoteke izbrane sezone.
- **Reset selected season**: ponastavi datoteke izbrane sezone.

### Aired episode catalogue

Za serijo se prikaže katalog predvajanih epizod ponudnika:

- **All episodes**: prikaže vse epizode s skupnim številom.
- **Missing**: prikaže manjkajoče epizode in njihovo število.
- **Uncertain**: prikaže negotove, podvojene, nedosegljive oziroma druge nejasne primere in njihovo število.
- **Season N**: razpre oziroma zapre posamezno sezono; navede tudi število epizod.
- Vsaka epizoda lahko pokaže številko, naslov, status, povzetek, datum predvajanja in pot lokalne datoteke. Naslov potrjeno prisotne epizode je gumb za predvajanje.
- Če katalog ni dosegljiv, se prikaže obvestilo in izbirnika uporabita lokalno prepoznane epizode.

### Bonus material

Če obstajajo bonusne datoteke, se prikaže razdelek **Bonus material**. Razpre seznam pripadajočih datotek; pri vsaki sta pot in **▶ Play**. Bonusna vsebina se ne šteje kot epizoda.

### Advanced settings · Local file details

Razdelek se razpre za pregled vseh datotek naslova. Za vsako prikazuje pot, dosegljivost, oznako ogledanosti, stanje ujemanja, pojasnilo identifikacije in zaznane podnapise:

- **Edit identification**: odpre ročni popravek identifikacije.
- **▶ Play**: predvaja to datoteko.
- **Watched**: označi posamezno datoteko kot ogledano.
- **Reset**: ponastavi položaj in ogledanost posamezne datoteke.

## 11. Edit identification

Okno pokaže pot izbrane datoteke in obrazec za izbiro pravilnih metapodatkov. Gumb je lahko viden tudi običajnemu uporabniku, vendar strežnik uporabniški vlogi ne dovoli iskanja in shranjevanja identifikacije; za popravek je potreben administrator ali lastnik:

- **Search for**: **Movie · TMDB** ali **TV show · TVmaze**.
- **Title**: iskalni naslov; začetno je izpolnjen z znanim oziroma iz imena izluščenim naslovom.
- **Search** oziroma Enter: poišče zadetke.
- Vsak zadetek je gumb z naslovom in morebitnim letom. Izbira filma shrani ujemanje; izbira serije odpre izbiro epizode.
- Pri seriji se izbere sezona in epizoda iz kataloga ter potrdi izbira. Ujemanje se shrani kot ročno potrjeno za prihodnja skeniranja.
- **Back to search** se lahko prikaže po napaki pri shranjevanju.
- **Close** zapre okno; prikazana so tudi stanja iskanja, nič zadetkov in napake.

## 12. Video predvajalnik

### Zgornji del

- **Back to library** oziroma puščica nazaj: zapre predvajalnik in se vrne v knjižnico oziroma prejšnji pogled.
- Naslov trenutnega videa.
- Pri epizodah izbirnika sezone in epizode, kadar je na voljo ustrezna serija; izbira preklopi na drugo lokalno epizodo.
- Sporočilo o pripravi, nalaganju, čakanju na disk, ponovnem povezovanju ali napaki.
- **Try again**: ob napaki ponovno poskusi predvajanje.

### Nadaljevanje predvajanja

Če obstaja primeren shranjeni položaj, se odpre **Continue watching?**:

- **Continue watching** nadaljuje na shranjenem položaju.
- **Start from beginning** začne na začetku in ponastavi shranjeni položaj.

### Časovnica

- Prikaz trenutnega in skupnega časa.
- Drsnik za skok na izbrani položaj.
- Ob premikanju kazalca po časovnici se lahko prikažeta čas in predogled sličice.

### Gumbi in izbirniki predvajanja

- **Play or pause**: predvajanje oziroma premor; klik na video prav tako preklopi predvajanje.
- **Back 10 seconds** in **Forward 10 seconds**: premik za deset sekund. Dvojni klik na levo oziroma desno stran videa prav tako premakne predvajanje.
- **Mute or unmute**: izklop oziroma vklop zvoka.
- **Volume**: nastavitev glasnosti.
- **Audio track**: izbira vgrajene zvočne sledi; prikaže se samo, če sta na voljo vsaj dve sledi. Sledi lahko vključujejo jezik, kodek, število kanalov in naslov. Sprememba lahko ponovno zažene pretvorbo pri trenutnem položaju.
- **Subtitles**: izklop ali izbira glavne besedilne podnapisne sledi.
- **Second subtitles**: izklop ali izbira druge podnapisne sledi; izbirnik se prikaže, kadar sta na voljo vsaj dve sledi.
- **Subtitle delay**: ročni zamik podnapisov v sekundah, od −30 do +30.
- **Quality**: **Auto**, **1080p**, **720p** ali **480p**.
- **Playback speed**: **0.5×**, **0.75×**, **1×**, **1.25×**, **1.5×** ali **2×**.
- **Convert**: ročno preklopi neposredno predvajanje na združljivo pretvorbo, kadar je možnost prikazana.
- **Stats**: pokaže oziroma skrije podatke o načinu predvajanja, razlogu za pretvorbo, kakovosti, kodekih, zvoku, izvirnem bitrateu, HDR, HLS, ocenjeni pasovni širini in medpomnilniku, kadar so podatki na voljo.
- **PiP**: poskusi vklopiti oziroma izklopiti sliko v sliki; ob nepodprtem načinu se prikaže sporočilo.
- **Fullscreen**: vklopi oziroma izklopi celozaslonski način.

### Naslednja epizoda

Ob koncu primerne epizode se pokaže okno **Next episode** z naslovom in podatki o naslednji lokalni epizodi:

- **Play now** začne naslednjo epizodo.
- **Keep watching** prekliče samodejni prehod.
- Če je v profilu vključeno samodejno nadaljevanje, se prikaže desetsekundno odštevanje; med premorom se ne odšteva.

### Tipkovnica

- **Space** ali **K**: predvajanje/premor.
- **J** ali leva puščica: deset sekund nazaj.
- **L** ali desna puščica: deset sekund naprej.
- **M**: utišanje oziroma vklop zvoka.
- **F**: celozaslonski način.
- **C**: menjava podnapisov.
- **Shift+N**: naslednja lokalna epizoda.

Ročnih označevalcev za preskok uvoda, povzetka ali odjavne špice, navigacije po poglavjih in gumba za prijavo težav ta različica nima.

## 13. Settings: nastavitve vsakega profila

### Profile preferences

Nastavitve se samodejno shranjujejo za aktivni profil:

- **Preferred playback quality**: privzeta kakovost, od **Auto** do posameznih fiksnih možnosti.
- **Playback speed**: privzeta hitrost predvajanja.
- **Subtitle language**: **Ask each time** ali izbrani jezik: angleščina, hrvaščina, slovenščina, nemščina, španščina, francoščina, italijanščina, japonščina, korejščina, portugalščina, ruščina ali kitajščina.
- **Automatically play the next episode**: vključi oziroma izključi samodejno nadaljevanje.
- **Continue watching items**: največje število postavk na polici za nadaljevanje.
- Ob shranjevanju se prikaže sporočilo o uspehu oziroma napaki.

### More profile settings

- **Preferred audio language**: prva/originalna sled ali prednostni jezik, če taka sled obstaja.
- **Subtitle behavior**: **Off by default**, **Use preferred language** ali **Always show a subtitle when available**.
- **Subtitle size**, **Subtitle background**, **Subtitle shadow** in **Subtitle color**: možnosti prikaza podnapisov.
- **Show Continue watching**, **Show recommendations** in **Show popular in your library**: posamezno vključijo oziroma izključijo ustrezne police v Overview.
- **Accent color** in **Reduce motion**: osebni možnosti videza oziroma zmanjšanja gibanja.

## 14. Settings: knjižnice in skeniranje

Ta del vidita administrator in lastnik.

### Libraries

- **Automatically sort recognized movies and shows into their matching libraries**: samodejno razporedi prepoznane filme in serije. Če je možnost izključena, gre pregledana vsebina v izbrano ciljno knjižnico.
- **Scan folders (one path per line)**: seznam strežniških map, ena pot na vrstico.
- **Save sorting and folders**: shrani samodejno razvrščanje in mape.
- **Browse folder…**: odpre sistemski izbirnik map na podprtem strežniku.

Za vsako obstoječo knjižnico:

- Polje **Library name** in izbirnik vrste **Movies**, **TV shows**, **Videos** ali **Other**.
- **Save**: shrani ime, vrsto in mape knjižnice.
- **Delete**: zahteva potrditev in izbriše knjižnico; medijske datoteke ostanejo, članstvo se prenese v drugo knjižnico iste vrste. Zadnje knjižnice posamezne vrste ni mogoče izbrisati.
- Število datotek in trenutna vrsta.
- **Folders (one per line)**: mape te knjižnice.
- **Browse…**: doda mapo z izbirnikom na podprtem strežniku.
- **Scan this library**: shrani trenutno knjižnico in začne pregled njenih navedenih map.

### Add a library

- Polje za ime nove knjižnice.
- Izbirnik vrste: **Videos**, **Movies**, **TV shows** ali **Other**.
- **Create**: ustvari knjižnico.

### Scan

- Izbirnik ciljne knjižnice za vsebino, ki se ji dodeli pregled.
- **Scan added or changed files**: inkrementalni pregled.
- **Full library scan**: ponovno preveri odklenjene datoteke.
- **Cancel scan**: zahteva ustavitev pregleda.
- Pojasnilo pove, da ročno potrjene identifikacije med ponovnim pregledom ostanejo zaščitene.

## 15. Settings: strežnik in ponudniki

Ta del vidi lastnik.

### Server

- **Server name** in **Save server name**: ime, prikazano v aplikaciji.
- **Transcoding encoder**: samodejna izbira oziroma razpoložljivi strojni kodirniki.
- **Save encoder**: shrani izbiro. Prikazano je tudi pojasnilo o programski rezervni možnosti.

### Metadata and subtitle providers

- **TMDB Read Access Token**: polje za token, stanje **configured** oziroma **not configured** in morebitno pojasnilo, da vrednost prihaja iz okolja. **Save TMDB token** shrani vneseni token.
- **OpenSubtitles API key**: enako stanje in polje za ključ. **Save OpenSubtitles key** shrani ključ.
- Obstoječi ključi se v obrazcu ne izpisujejo. Prikazan je rezultat shranjevanja ali napaka.

### Transcoding quality

- **Balance**: **Faster start · lower bitrate**, **Balanced** ali **Higher picture quality · more CPU and bandwidth**.
- **Save transcoding balance**: shrani skupno nastavitev pretvorbe na strežniku.

## 16. Settings: prijavljene naprave

**Signed-in devices** je na voljo uporabniku za njegov račun:

- **Show sessions**: izpiše trenutne seje, označi **This device** oziroma **Other session** ter prikaže čas prijave in poteka.
- **Sign out other devices**: odjavi druge seje tega računa.
- Prikaže se lahko prazno stanje ali napaka pri nalaganju.

## 17. Settings: podnapisi

Razdelek **Subtitles** vsebuje **Search subtitles**. Običajni uporabnik vidi obrazec, vendar strežnik njegovo spletno iskanje OpenSubtitles zavrne; lokalni seznam je lahko prikazan iz že naloženih podatkov:

- Pojasnilo o lokalno zaznanih podnapisih in iskanju po OpenSubtitles; spletni ponudnik prejme iskalni naslov.
- Gumb **Settings** ob pojasnilu o ponudniku odpre nastavitve.
- **Title**: iskalni naslov.
- **Subtitle language**: **All languages** ali eden od ponujenih jezikov.
- **Search** oziroma Enter: poišče lokalne in spletne rezultate.
- **Detected on your computer**: ujemajoče se lokalne datoteke s potjo in imeni zaznanih podnapisov.
- **OpenSubtitles results**: izdaja oziroma ime datoteke, jezik, ocena in število ocen, število prenosov ter morebitna oznaka **Hearing impaired**.
- **View on OpenSubtitles**: odpre rezultat pri ponudniku, če je povezava veljavna.
- Izpišejo se tudi stanja brez rezultatov in napake. Ta del ne zagotavlja samodejnega prenosa ali sinhronizacije podnapisov.

## 18. Settings: diagnostika in računi

### Diagnostics – administrator in lastnik

- **Check server health** preveri orodji FFmpeg in ffprobe, konfiguracijo TMDB, število in skupno količino medijskih datotek ter dostopnost knjižničnih map.
- **Library folders**: seznam map in njihova dostopnost.
- **Needs attention**: razširljiv seznam do 100 naslovov, ki potrebujejo pregled, s potjo in razlogom.
- **Largest files**: razširljiv seznam največjih datotek.
- Med preverjanjem oziroma ob napaki se prikaže ustrezno sporočilo.

### Server activity – administrator in lastnik

- **Accounts**, **Active streams** in **Estimated data**: števci računov in aktivnih predvajanj ter ocena prenesenih podatkov.
- **Currently watching**: za vsako dejavno predvajanje prikaže uporabnika, profil, datoteko, način, položaj in čas gledanja. Ocena podatkov ni dejanska meritev omrežnega prometa.
- Če ni nedavne dejavnosti, se prikaže prazno stanje.

### Accounts – administrator in lastnik

Seznam prikaže uporabniško ime, vlogo, e-poštni naslov in število profilov.

Lastnik pri posameznem računu dodatno vidi:

- **Email address**, **New password**, **Confirm new password**.
- Pri lastnem računu še **Current owner password** in navodilo za obnovitev pozabljenega gesla v terminalu.
- **Save credentials**: shrani nove poverilnice; sprememba odjavi obstoječe seje tega računa.

### Add an account – administrator in lastnik

- **Email address**, neobvezni **Display name**, **Temporary password** in **Confirm password**.
- **Role**: **User · can watch media**; lastnik lahko izbere tudi **Administrator · can manage the server**.
- **Create account**: ustvari račun in začetni profil. Izpiše se potrditev ali napaka.

## 19. Pomoč in nedosegljiv strežnik

- **Help & documentation** odpre samostojno stran z navodili za uporabo in vzdrževanje.
- Če je bila javna offline stran že shranjena in strežnik ni dosegljiv, brskalnik lahko pokaže stran **Your media server is unavailable** z navodilom za ponovno povezavo. To ni offline dostop do zasebne knjižnice ali videov.
- Ob neuspešnem nalaganju prijavljene seje se lahko prikaže **Could not load your library** z gumboma **Try again** in **Return to sign in**.

## 20. Razlike med vlogami

- **User**: uporablja knjižnice, iskanje, predvajalnik, profile, Watchlist, nastavitve profila, razpoložljive podnapise v predvajalniku in svoje seje. Gumbi za urejanje identifikacije in spletno iskanje podnapisov so lahko vidni, vendar strežnik ta dejanja uporabniški vlogi zavrne.
- **Administrator**: poleg tega upravlja knjižnice in skeniranje, vidi diagnostiko in dejavnost ter lahko ustvari navadni uporabniški račun.
- **Owner**: poleg administratorskih možnosti upravlja nastavitve strežnika, ponudniške ključe in poverilnice računov ter lahko ustvari administratorja.

Posamezni gumbi se lahko pojavijo šele po nalaganju podatkov oziroma ko so ustrezne datoteke, epizode, sledi, profili ali dovoljenja dejansko na voljo.
