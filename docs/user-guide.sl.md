# PlexV2: navodila za uporabo

PlexV2 je lokalna video knjižnica s spletnim predvajalnikom. Strežnik prebere izbrane mape, poskuša prepoznati filme in serije ter omogoča predvajanje v brskalniku. Medijskih datotek pri skeniranju in predvajanju ne preimenuje, premika ali prepisuje.

## Prvi zagon

Potrebujete Rust s Cargo ter FFmpeg in ffprobe, dostopna strežniku. Iz korena projekta zaženite:

```sh
cargo run --release --locked
```

Odprite http://127.0.0.1:8787 in ustvarite lastniški račun. Vnesite veljaven e-poštni naslov, geslo z vsaj 12 znaki in potrditev gesla. Prikazno ime je lahko drugačno od e-pošte. Strežnik mora med uporabo ostati zagnan.

Lastnik obstaja samo enkrat. Naslednje račune dodaja lastnik oziroma administrator v Settings. Lastnik lahko ustvari administratorja; administrator lahko ustvarja navadne uporabnike.

Podatkovni imenik je privzeto sistemski podatkovni imenik aplikacije smart-media-scanner. SMART_MEDIA_DATA ga lahko spremeni. Ne uporabljajte medijske mape kot podatkovnega imenika.

## Knjižnice in mape

V Settings poiščite Libraries. Knjižnico lahko ustvarite, preimenujete in določite njen tip: Movies, TV shows, Videos ali Other. Vrsti Videos in Other omogočata organizacijo poljubnih videov; nista predvajalnika glasbe ali fotografij.

V polje Folders vnesite eno absolutno pot na vrstico. Pot mora obstajati na računalniku oziroma v vsebniku, kjer teče strežnik. Pot na telefonu ali drugem računalniku ni pot na strežniku.

Primeri poti:

```text
/Volumes/Media/Movies
/home/media/TV
D:\Media\Movies
```

Browse odpre sistemski izbirnik na macOS. Na drugih sistemih poti vnesite ročno. V Docker namestitvi iz primera uporabite /media; gostiteljeve poti znotraj vsebnika niso samodejno enake.

Auto-sort po prepoznavi razporeja filme in serije v ustrezni knjižnici. Drugi videi se dodelijo izbrani ciljni knjižnici. Če Auto-sort izklopite, morate izbrati ciljno knjižnico; vsi pregledani videi gredo vanjo.

Brisanje knjižnice ne izbriše videov. Članstvo datotek se prenese v drugo knjižnico istega tipa. Zadnje knjižnice posameznega tipa ni mogoče izbrisati; lahko jo preimenujete.

## Skeniranje

Scan added or changed files pregleda izbrane mape in preskoči nespremenjene datoteke. Full library scan ponovno preveri odklenjene datoteke. Ročno potrjene identifikacije ostanejo zaščitene v obeh primerih.

Skeniranje teče na strežniku tudi po zaprtju zavihka. Napredek, trenutna datoteka in zadnje dejavnosti so prikazani v obvestilu na vrhu strani. Cancel zahteva ustavitev; že začeta omrežna poizvedba se lahko najprej zaključi.

Odklopljen disk ne pomeni, da so datoteke izbrisane. Neuspešno dostopna mapa ne izbriše prejšnjih zapisov. Po zagonu je dosegljivost lahko začasno neznana, dokler se ne konča preverjanje.

## Prepoznavanje filmov in serij

Serije uporabljajo TVmaze, filmi TMDB. Za filme v lastniških nastavitvah dodajte TMDB API Read Access Token. To ni isto kot krajši v3 API key. Token se hrani na strežniku in se v obrazcu ne izpiše nazaj.

Pri serijah pomagajo imena s sezono in epizodo:

```text
TV/Example Show/Season 01/Example.Show.S01E02.mkv
TV/Example Show/Season 01/Example.Show.S01E03-E04.mkv
Movies/Example Movie (2020)/Example.Movie.2020.mkv
```

Rezultat ujemanja ni zagotovilo pravilnosti vsebine. Če je naslov napačen, odprite njegove podrobnosti oziroma Advanced settings in izberite Edit identification. Poiščite pravi naslov in potrdite zadetek; pri seriji izberite epizodo v vrstnem redu TVmaze. Potrditev zaklene izbiro za naslednje preglede.

Katalog serije prikazuje prisotne, manjkajoče in negotove epizode. Zelena označuje potrjeno prisotnost, rdeča manjkajočo epizodo, rumena pa tudi nedosegljivost, podvojitev ali drug negotov status. Filtri All, Missing in Uncertain odprejo ustrezne sezone. Katalog ponudnika je lahko nepopoln.

## Brskanje, iskanje in filtri

Libraries odpre seznam knjižnic v stranskem meniju. V knjižnici filmov ali serij lahko filtrirate po letu, žanru, jeziku, minimalni oceni, trajanju in nepogledanosti. Filtri uporabljajo znane podatke. Če trajanje ali žanr manjka, naslov ne ustreza aktivnemu filtru tega podatka.

Sort podpira naslov, leto, oceno, Recently added in Shuffle. Recently added trenutno uporablja čas zadnjega skeniranja oziroma mtime, ne nespremenljivega datuma prvega dodajanja.

Surprise me naključno izbere naslov med trenutnimi filtriranimi zadetki. Če zadetkov ni, ni česa izbrati. Globalno iskanje išče po normaliziranih naslovih in imenih oziroma poteh datotek; ni iskanje po igralcih in ni samodejno odporno na vse tipkarske napake.

Collections vsebujejo tudi lokalne skupine, sklepane iz map in oštevilčenih datotek. Taka skupina sama po sebi ni potrjen katalog epizod.

## Watchlist in zgodovina ogledov

V podrobnostih naslova uporabite Add to Watchlist. Watchlist je shranjen na strežniku za aktivni profil. Remove odstrani naslov s seznama; ne izbriše datoteke in ne ponastavi zgodovine ogledov.

Mark movie/series watched označi lokalne datoteke kot ogledane. Pri seriji lahko označite izbrano sezono z gumbom ob izbirniku sezone ali neposredno v vrstici sezone v katalogu. Dejanje zajame samo lokalne datoteke te sezone. V naprednih podrobnostih obstajajo tudi dejanja za posamezno datoteko.

Mark unwatched / reset je prikazan samo, kadar je vsaj ena od izbranih lokalnih datotek označena kot ogledana. Gumb odstrani shranjeni položaj in oznako ogledanosti za izbrane datoteke. To vpliva tudi na Continue Watching. Ponastavitev ne posega v zgodovino drugih profilov.

Continue Watching ponudi nadaljevanje shranjenega položaja. Ob ponovnem odpiranju videa lahko izberete Continue watching ali Start from beginning. Položaj se periodično shranjuje in dodatno pošlje ob zaprtju predvajalnika ali skrivanju zavihka. Ob nenadni izgubi napajanja zadnja sekunda ni zagotovljena.

## Predvajanje in kakovost

Play odpre predvajalnik. Združljivi H.264/AAC MP4 se pošiljajo neposredno z HTTP byte-range zahtevami. Drugi formati se po potrebi pretvorijo v H.264/AAC. Predvajanje ne spremeni izvirnika.

Auto pri pretvorbi HLS pripravi različice 480p, 720p in 1080p; predvajalnik izbira kakovost glede na razmere. Tri različice hkrati porabijo več procesorja. Na šibkem strežniku izberite fiksno kakovost 480p ali 720p. Pretvorba ne ustvari podrobnosti, ki jih ni v izvirniku.

Če neposredno predvajanje odpove, predvajalnik poskusi združljivo pretvorbo. Compatibility mode ročno vključi programsko pretvorbo H.264/AAC pri trenutnem položaju. Pri Auto uporabi fiksno 720p in ob slabem indeksu datoteko dekodira od začetka do izbranega časa. To lahko traja dlje, vendar obide nedelujoče hitro iskanje. FFmpeg mora podpirati vhodni kodek; šifriranih ali nepopravljivo poškodovanih datotek ni mogoče zagotovljeno predvajati. Dolgo nalaganje je lahko posledica odklopljenega diska, omrežne mape ali počasne pretvorbe; počakajte na sporočilo in uporabite Try again.

HLS in progresivna pretvorba si delita največ štiri sočasna mesta na strežniku. Sporočilo o zasedenih mestih pomeni, da zaprite neuporabljeno predvajanje oziroma poskusite pozneje. To ni čakalna vrsta ali omejitev po računu.

HDR PQ/HLG se med pretvorbo preslika v SDR. FFmpeg mora vsebovati filtra zscale in tonemap. Pravilnost barv na vsakem dejanskem HDR posnetku ali zaslonu ni zagotovljena samo s tem, da je datoteka označena kot HDR.

## Zvok

Izbirnik Audio track je viden samo, ko ima datoteka vsaj dve zvočni sledi. Med začetnim branjem informacij in pri nič ali eni sledi je skrit.

Več sledi je običajno vgrajenih v isto MKV ali MP4 datoteko, na primer izvirnik, sinhronizacija in komentar. Izbirnik prikaže jezik, kodek, kanale in naslov, če so zapisani. Ločenih zunanjih AC3/AAC datotek ni mogoče pripeti kot nadomestni zvok.

Menjava zvoka ponovno zažene pretvorbo pri trenutnem položaju in lahko kratko nalaga. Preferred audio language v nastavitvah profila izbere ujemajočo se sled, če obstaja; sicer se uporabi prva. Izbira jezika ne ustvari manjkajoče sinhronizacije.

## Podnapisi

Predvajalnik podpira vgrajene besedilne sledi in zaznane lokalne SRT, VTT, ASS ali SSA ob video datoteki. Napredne ASS animacije in oblikovanje se pri pretvorbi v WebVTT ne ohranijo v celoti. Slikovni PGS/VobSub niso podprti.

Izberite glavno sled v Subtitles. Če je na voljo več sledi, lahko dodate Second subtitles; dve različni sledi sta prikazani na ločenih višinah. Ista sled se ne prikazuje dvakrat.

Subtitle delay je ročni zamik v sekundah med -30 in 30. Pozitivna vrednost pomeni, da se podnapis pokaže pozneje; negativna prej. Spremembe se ne seštevajo: prehod z +2 na +1 pomeni končni zamik +1. Zamik ne spremeni izvirne datoteke.

V nastavitvah profila izberete privzeti jezik in način: off, preferred language ali always, ter velikost, barvo, ozadje in senco. Privzeti jezik deluje samo, če je sled dovolj dobro označena.

Settings vsebuje iskanje lokalnih podnapisov in OpenSubtitles. Za spletno iskanje lastnik nastavi aplikacijski API ključ. Trenutna funkcija išče in poveže rezultat pri ponudniku; ne zagotavlja samodejnega prenosa, ujemanja ali sinhronizacije.

Samodejno zaznavanje in popravljanje razmaknjenih podnapisov je zapisano v TODO. Trenutno uporabite ročni zamik ali preverjen zunanji pripomoček na kopiji podnapisa.

## Tipkovnica, PiP in naslednja epizoda

Space ali K preklopi predvajanje/premor. Puščica levo ali J preskoči nazaj 10 sekund; puščica desno ali L naprej 10 sekund. F preklopi fullscreen, M utišanje, C kroži med podnapisi, Shift+N odpre naslednjo lokalno dosegljivo epizodo. Ctrl+K oziroma Cmd+K fokusira iskanje knjižnice.

Med tipkanjem v vnosna polja, selecte in besedilna polja se bližnjice predvajalnika ne sprožijo. Kartice knjižnice odprete tudi s tipkama Enter ali Space, ko imajo fokus.

PiP uporabi podporo brskalnika in operacijskega sistema. Če podpore ni ali video še ni pripravljen, se prikaže sporočilo. Ni zagotovljen na vseh napravah.

Ob koncu epizode se pokaže kartica naslednje lokalne epizode. Če je Auto-play next vključen v profilu, se začne desetsekundno odštevanje. Play now začne takoj; Keep watching prekliče samodejni prehod. Med premorom odštevanje počaka. Manjkajočih epizod aplikacija ne prenese.

Ročnih skip markerjev in navigacije po poglavjih v trenutnem predvajalniku ni.

## Profili in računi

Profil izberete prek avatarja. Add profile zahteva ime in dvakrat vnesen PIN s 4 do 8 številkami. Preklop na profil zahteva njegov PIN; pri starejšem profilu brez PIN-a ga najprej nastavite. Po petih napačnih poskusih sledi petnajstminutna omejitev.

Profil ima svojo zgodovino, watchlist in nastavitve. To ni starševski nadzor. PIN sam ne pomeni starostnih omejitev, različnih dovoljenj za knjižnice ali zaščite pred lastnikom računa.

V Settings pod Signed-in devices izberite Show sessions. Vidite seje svojega računa in lahko izberete Sign out other devices. To ne razkriva sej drugih računov. Sign out odjavi trenutno napravo.

Lastnik spreminja računske podatke v administratorskem delu. Pri spremembi svojih poverilnic mora navesti trenutno geslo. Sprememba gesla odjavi obstoječe seje tega računa.

Če je lastnik geslo pozabil, ustavite strežnik in v terminalu zaženite interaktivno obnovitev z istim SMART_MEDIA_DATA:

```sh
./target/release/smart-media-scanner reset-owner-password
```

## Diagnostika in odpravljanje težav

Stats v predvajalniku pokaže trenutni način, razlog za pretvorbo, kodeke in podatek o kakovosti. Podatki so namenjeni razumevanju toka; ocenjeni bitrate ali medpomnilnik nista meritev dejanske kakovosti slike.

Administrator v Settings odpre Check server health: preveri FFmpeg, ffprobe, TMDB konfiguracijo in knjižnične mape ter prikaže število in skupno velikost datotek, največje datoteke in do sto neprepoznanih datotek. Needs attention ni dokaz poškodbe datoteke.

Server activity prikazuje aktivna predvajanja. Količina podatkov je ocena iz velikosti datoteke in položaja, ne merjen omrežni promet. Obvestilo skeniranja vsebuje ločen dnevnik njegovega napredka.

Če videa ni mogoče odpreti, preverite povezavo diska, strežniško pot in dovoljenja. Če se video zatika, poskusite nižjo fiksno kakovost in preverite obremenitev računalnika. Če spletna stran ni dosegljiva, preverite, da strežnik teče, vrata in dostop prek domačega omrežja ali VPN.

Vgrajene prijave težav trenutno ni. Pri prijavi na GitHub opišite korake, pričakovano in dejansko vedenje ter različice sistema, brskalnika in FFmpeg. Pred objavo odstranite zasebne poti, tokene in podatke računov. Lokalni administrator ni nujno razvijalec projekta.

## Mobilne naprave, PWA in oddaljeni dostop

Spletni vmesnik je prilagojen manjšim zaslonom; mobilni predvajalnik uporabi celoten razpoložljivi viewport. Fizični telefoni in TV daljinci niso enakovredni preizkusu velikosti okna v Chromu.

Na podprtem brskalniku lahko aplikacijo namestite prek njegovega menija. Potrebujete HTTPS ali localhost. PWA predpomni samo javni zaslon za nedosegljiv strežnik, ikone in manifest. Zasebne knjižnice in video niso shranjeni za offline ogled.

Za dostop zunaj računalnika uporabite ustrezno nastavljen HTTPS reverse proxy ali zasebni VPN. SMART_MEDIA_COOKIE_SECURE=1 je za HTTPS; pri navadnem HTTP taki piškotki ne omogočajo običajne prijave. Podrobnosti so v dokumentu docs/deployment.md v repozitoriju.

## Posodobitev in varnostna kopija

Po spremembah HTML/JS je potrebna nova Rust gradnja, ker je spletni vmesnik vgrajen v izvršno datoteko. Nato ustavite staro instanco in zaženite novo z istimi podatkovnimi nastavitvami. Samo osvežitev brskalnika ne naloži nove kode v že zagnani strežnik.

Pred varnostno kopijo ustavite strežnik in kopirajte celoten podatkovni imenik, skupaj z zasebnimi provider datotekami. Med živim SQLite WAL delovanjem ne kopirajte samo library.sqlite3. Kopije vsebujejo zasebne podatke; hranite jih z ustreznimi dovoljenji.

Ne brišite podatkovnega imenika kot običajen korak odpravljanja težav: s tem izgubite račune, ročne identifikacije in zgodovino. Za obnovitev uporabite preverjeno kopijo z ustavljenim strežnikom.

## Samodejna sinhronizacija podnapisov z govorom

Strežnik lahko iz kratkih zvočnih vzorcev lokalno ustvari prepis s časovnimi oznakami in ga primerja z izbranimi besedilnimi podnapisi. Potrebuje whisper-cli in model ggml-base.bin. Na macOS namestite orodje z brew install whisper-cpp ter zaženite python3 scripts/setup-subtitle-sync.py. Pri lastnem podatkovnem imeniku dodajte --data-dir ali nastavite SMART_MEDIA_WHISPER_MODEL; SMART_MEDIA_WHISPER določi pot do orodja. Model se prenese samo pri izrecnem zagonu namestitvene skripte, s preverjanjem SHA-256.

Automatic subtitle sync v nastavitvah profila je privzeto vključen, kadar je lokalni pogon nameščen. Ob izbiri podnapisov analizira največ dva 75-sekundna vzorca. Najmanj trije različni ujemajoči se odlomki govora določijo zamik; pri daljšem videu lahko drugi vzorec oceni tudi postopno časovno drsenje. Slabo ujemanje, prevedeni podnapisi ali druga montaža lahko preprečijo zanesljivo korekcijo; dotedanje časovne nastavitve ostanejo.

Sync subtitles ponovno poskusi uskladitev. Cancel sync prekine opravilo trenutnega profila. Undo sync obnovi prvotne časovne oznake med trenutnim predvajanjem. Ročni Subtitle delay se prišteje k uskladitvi. Nastavitev je ločena za obe prikazani podnapisni sledi in sledi preskokom v videu. Izvirne datoteke se ne prepišejo. Vzorci in prepisi ostanejo na strežniku v začasnem imeniku; predpomnjena je samo ugotovljena časovna korekcija, ločeno po profilu, videu, podnapisu, zvoku in modelu.
