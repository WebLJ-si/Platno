# Presoja predlogov funkcij

Pregled izvornega dokumenta z dne 29. 10. 2026. Dokument je seznam idej; njegove interne prioritete niso dodatna navodila uporabnika. Vsak predlog spodaj ima izrecen status. »Odloženo« pomeni smiseln ločen razvojni sklop, ne implementirane funkcije.

Funkcije smo preverjali z Rust testi, izoliranim HTTP/FFmpeg okoljem in Chrome UI preizkusom. Za dejanske omejitve testiranja glej `validation.md`.

Posodobitev 29. 10. 2026: po zahtevi uporabnika so ročni skip markerji, navigacija po poglavjih in lokalne prijave težav odstranjeni. Izbira zvoka ostaja vidna samo pri več sledih. Samodejno ujemanje/sinhronizacija podnapisov in prihodnji maintainer issue workflow sta v `TODO.md`.

## Najprej: 12 funkcij z največjim učinkom

| Predlog | Presoja | Izvedba / razlog |
| --- | --- | --- |
| Izbira zvočne sledi. V popisu je označena kot neimplementirana, a je nujna: sinhronizacije, večjezični filmi, komentarji režiserja. | Uvedeno | Izbira zvočne sledi z jezikom/kodekom/kanali v HLS in progresivni pretvorbi. |
| Skip Intro / Recap / Credits. Gumb med predvajanjem. Intro se zazna z ffmpeg (primerjava zvoka ali črnih okvirjev med epizodami) ali z ročno označbo časa. | Odstranjeno po zahtevi | Ročni markerji in njihovi API-ji so odstranjeni. Samodejnega zaznavanja ni. |
| Naslednja epizoda z odštevanjem. Ob koncu špice se prikaže kartica z odštevanjem, kot pri Netflixu (post-play). | Uvedeno | Kartica, 10-sekundno odštevanje po nastavitvi profila, preklic in ročni začetek. |
| Mark as watched / unwatched. Z enim klikom za film, epizodo, sezono ali serijo, plus ponastavitev napredka. | Uvedeno | Transakcijsko označevanje filma/datoteke/sezone/serije; unwatched izbriše položaj. |
| HLS adaptive bitrate. Več kakovosti v master playlisti, da hls.js sam preklaplja glede na hitrost omrežja. | Uvedeno | Master playlist s 480/720/1080; hls.js izbira glede na omrežje. Native HLS izbira odjemalec. |
| Burn-in slikovnih podnapisov (PGS/VobSub). Pri Blu-ray remuxih zelo pogosti, zdaj niso podprti. | Odloženo | Potrebuje potrjene PGS in VobSub vzorce, preverjanje geometrije/barv in kombinacij seek/ABR. Trenutna izdaja ohranja besedilne podnapise. |
| HDR → SDR tone mapping. Brez njega so HDR filmi po pretvorbi zbledeli. | Uvedeno | Samodejno zaznavanje PQ/HLG in zscale/tonemap v obeh pretvornikih; zahteva ustrezna FFmpeg filtra. |
| Trickplay. Sličice predogleda ob vlečenju drsnika, generirane z ffmpeg. | Delno uvedeno | Predogledi pri drsniku nastajajo na zahtevo v 10-sekundnih korakih; predpomnilnik je omejen. Vnaprejšnje generiranje sprite atlasov ostaja za pozneje. |
| Starševski nadzor. Starostne omejitve po profilu (G/PG/12/16/18), otroški profil in zaklep knjižnic po profilu. | Odloženo | Varnostno celovit sklop: potrebuje zanesljive starostne oznake ter strežniško omejitev vseh seznamov, metapodatkov, tokov in profilnih prehodov. Skrivanje UI bi ustvarilo lažen občutek zaščite. |
| Varen oddaljen dostop. Navodila ali vgrajena podpora za reverse proxy in HTTPS, po možnosti Cloudflare Tunnel ali Tailscale. | Uvedeno | Navodila in Caddy primer za HTTPS ter zasebni VPN; brez samodejnega odpiranja portov ali ustvarjanja tunelov. |
| Bližnjice na tipkovnici. Presledek, ←/→, F, M, J/L, C za podnapise, Shift+N za naslednjo epizodo. | Uvedeno | Space/K, puščice/J/L, F, M, C, Shift+N; Ctrl/Cmd+K za iskanje. Vnosna polja ne sprožajo ukazov. |
| Odzivnost za mobilne naprave in TV ter PWA. Namestljiva aplikacija z ikono, manifestom in prikazom na celem zaslonu. | Delno uvedeno | Manifest, PNG ikoni, service worker, zaslon za nedosegljiv strežnik in prilagoditev mobilnih kontrol. Namestitev zahteva HTTPS ali localhost. Fizični TV odjemalec in daljinec nista preverjena. |

## Predvajalnik

| Predlog | Presoja | Izvedba / razlog |
| --- | --- | --- |
| Ločen izbirnik zvoka in podnapisov z oznakami jezika in kodeka (npr. "Angleščina · EAC3 5.1") | Uvedeno | Izbira zvočne sledi z jezikom/kodekom/kanali v HLS in progresivni pretvorbi. |
| Privzeta jezika zvoka in podnapisov po profilu (npr. zvok v izvirniku, podnapisi v slovenščini) | Uvedeno | Jezik zvoka po profilu in obstoječe nastavitve jezika/načina podnapisov. |
| Picture-in-Picture in mini predvajalnik | Delno uvedeno | Brskalniški PiP z WebKit fallbackom; podpora je odvisna od brskalnika. Mini predvajalnik med brskanjem še ni uveden. |
| Chromecast, AirPlay in DLNA predvajanje | Odloženo | Vsak protokol potrebuje odkrivanje naprav, receiver/sejo, varen dostop do toka in preizkus na fizični napravi. |
| Poglavja (chapters) iz MKV/MP4 z oznakami na časovnici | Odstranjeno po zahtevi | Izbirnik poglavij in njegovi podatki v playback info so odstranjeni. |
| Zapomni si zadnjo hitrost in glasnost | Uvedeno | Hitrost in glasnost se shranjujeta v profil; glasnost tudi lokalno kot fallback. |
| Normalizacija glasnosti in nočni način (dinamična kompresija): tiho govorjenje, glasne eksplozije | Odloženo | Potrebuje preverjeno zvočno verigo za oba pretvornika in meritve glasnosti; ne spreminjamo zvoka brez jasne nastavitve. |
| Prostorski zvok: ohranitev 5.1 in downmix v stereo, če odjemalec ne zmore | Delno obstoječe | Združljivi izvirniki ostanejo nespremenjeni; HLS downmix v stereo je preverjen. Večkanalna dostava zahteva odjemalski capability handshake. |
| Ročni zamik podnapisov (+/- sekunde) in zamik zvoka | Delno uvedeno | Ročni zamik besedilnih podnapisov ±30 sekund; zamik zvoka še ni uveden. |
| Stil podnapisov: ozadje, rob, senca, položaj | Delno uvedeno | Velikost, barva, temno/prosojno ozadje in senca. Dvojni podnapisi imajo ločen položaj; poljubnega urejevalnika sloga še ni. |
| Dvojni podnapisi hkrati (učenje jezikov) | Uvedeno | Dve različni besedilni sledi s skupnim časovnim zamikom in ločenima položajema. |
| Prikaz "Statistika za nerde": kodek, bitrate, direct/transcode, razlog za transkodiranje | Uvedeno | Podatki toka, izbrana sled, kakovost, način in razlog za pretvorbo v predvajalniku. |
| Gumb "Poročaj o težavi" ob predvajanju, ki zabeleži podatke o toku | Odstranjeno po zahtevi | Odstranjeni UI, API, lokalno shranjevanje prijav in administratorski seznam. Prihodnji workflow je v TODO.md. |
| Preskok ±10 s z dvojnim klikom ali dotikom (mobilno) | Delno uvedeno | Dvojni klik na levo/desno polovico preskoči ±10 s. Mobilne drsne geste niso uvedene. |
| Gesture kontrole: svetlost, glasnost, preskok z drsenjem | Odloženo | Potrebuje preizkuse konfliktov s scrollom, sistemskimi gestami in dostopnostjo na dejanskih telefonih. |
| Shranjevanje položaja tudi ob zaprtju zavihka (sendBeacon) | Uvedeno | Položaj se pošlje ob close/pagehide/visibilitychange; keepalive fallback in profilni lokalni ključ. |
| Ambilight / atmosferski način ozadja | Odloženo | Dekorativna funkcija z dodatno GPU porabo; ni nujna za zanesljivost predvajanja. |

## Iskanje, brskanje in odkrivanje

| Predlog | Presoja | Izvedba / razlog |
| --- | --- | --- |
| Filtri: žanr, leto, ocena, jezik, država, studio, trajanje, nepogledano | Delno uvedeno | Žanr, leto, ocena, jezik, trajanje, nepogledano in razvrščanje. Država/studio še nimata enotnih shranjenih podatkov. Neznane vrednosti so jasno izključene iz aktivnih filtrov. |
| Razvrščanje: dodano, leto, ocena, abecedno, naključno | Uvedeno | Naslov, datum skeniranja/mtime, leto, ocena in stabilno naključno razvrščanje. Nespremenljiv datum prvega dodajanja še ni shranjen. |
| Iskanje po igralcih, režiserjih in scenaristih (stran osebe s filmografijo) | Odloženo | Potrebuje shranjene credits/osebe in provider povezave; trenutni indeks teh podatkov nima. |
| Stran igralca: vsi filmi, ki jih imaš lokalno | Odloženo | Potrebuje shranjene credits/osebe in provider povezave; trenutni indeks teh podatkov nima. |
| Trailerji: samodejni predogled ob hoverju, kot pri Netflixu, ali gumb "Trailer" | Odloženo | Potrebuje ločene provider video reference in odločitve o zunanjem predvajanju/zasebnosti. Samodejni hover video ni privzeta izboljšava. |
| "Več podobnega" (More like this) iz žanra, igralske zasedbe in ključnih besed TMDB | Delno obstoječe | Lokalna priporočila iz opisov in zgodovine že obstajajo. Credits/keywords niso shranjeni, zato njihova uporaba še ni mogoča. |
| Polica "Ker si gledal X" | Delno obstoječe | Lokalna priporočila iz opisov in zgodovine že obstajajo. Credits/keywords niso shranjeni, zato njihova uporaba še ni mogoča. |
| Naključni film ("Presenečenje me") z izbiro razpoloženja ali žanra | Delno uvedeno | Surprise me upošteva aktivne filtre; žanr deluje ob znanih metapodatkih. Razpoloženje ni modelirano. |
| Ročne zbirke, ki jih ustvari uporabnik | Odloženo | Potrebuje lasten model članstva, urejanje in profilna dovoljenja. Obstoječe TMDB/lokalne zbirke ostajajo. |
| Pametne zbirke z pravili (npr. "komedije po 2010 z oceno nad 7") | Odloženo | Potrebuje lasten model članstva, urejanje in profilna dovoljenja. Obstoječe TMDB/lokalne zbirke ostajajo. |
| Seznami predvajanja (playlists), skupni ali osebni | Odloženo | Potrebuje lasten model članstva, urejanje in profilna dovoljenja. Obstoječe TMDB/lokalne zbirke ostajajo. |
| Oznake (tags/labels) in ročno razvrščanje | Odloženo | Potrebuje lasten model članstva, urejanje in profilna dovoljenja. Obstoječe TMDB/lokalne zbirke ostajajo. |
| Polici "Pravkar dodano" in "Novo ta teden" | Odloženo | Čas prvega dodajanja mora biti ločen od zadnjega skeniranja; razvrščanje po obstoječih podatkih je uvedeno, napačne police ne prikazujemo. |
| Polica "Zadnje gledano" z možnostjo odstranitve iz Continue Watching | Delno uvedeno | Continue Watching in profilna zgodovina obstajata; reset odstrani položaj. Ločena zgodovinska polica še ni uvedena. |
| Polica "Top 10 v tvoji knjižnici" po zgodovini ogledov celotnega strežnika | Odloženo | Sedanja zgodovina hrani zadnje stanje, ne vseh dogodkov ogledov; iz nje ni pošteno izračunati strežniškega Top 10. |
| Ocene IMDb, Rotten Tomatoes in Metacritic poleg TMDB | Odloženo | Potrebuje dodatnega ponudnika/ključ in preverjene pogoje uporabe. TMDB/TVmaze ocene ostajajo. |
| Prikaz starostne oznake in kratkih opozoril (nasilje, jezik) | Odloženo | Manjka zanesljiv model regionalnih oznak in opozoril; povezano s starševskim nadzorom. |
| Izbor po času ("imam 90 minut") | Uvedeno | Filter časa 30/60/90/120/180 minut upošteva le znano trajanje. |
| Globalno iskanje s Ctrl+K in predlogi med tipkanjem | Delno uvedeno | Ctrl/Cmd+K fokusira obstoječe globalno iskanje; ločeno okno s predlogi še ni uvedeno. |
| Iskanje, odporno na tipkarske napake (fuzzy) | Odloženo | Potrebna sta relevančno razvrščanje in preizkus velikih knjižnic; obstoječe normalizirano iskanje naslovov/datotek ostaja. |
| Hero karusel ("billboard") na Overview s tihim predvajanjem v ozadju | Delno obstoječe | Overview ima statični hero. Vrtenje in samodejno predvajanje sta dekorativna in zahtevata nadzor gibanja ter stroškov omrežja. |

## Metapodatki in knjižnica

| Predlog | Presoja | Izvedba / razlog |
| --- | --- | --- |
| Več ponudnikov: poleg TMDB še Trakt, OMDb, Fanart.tv in TheTVDB (za serije) | Odloženo | Nov adapter, identitete, cache, naročnine in pravila razreševanja nasprotujočih se rezultatov so samostojen razvojni sklop. |
| Podpora za anime (AniDB/AniList, absolutna številka epizod) | Odloženo | Nov adapter, identitete, cache, naročnine in pravila razreševanja nasprotujočih se rezultatov so samostojen razvojni sklop. |
| Glasbena knjižnica (albumi, izvajalci, seznami, besedila) in predvajalnik zvoka v ozadju | Odloženo | Nove vrste medijev potrebujejo lastne modele, metapodatke in odjemalce. Poljubne video knjižnice so že podprte. |
| Knjižnica fotografij in domačih videov (datumi, albumi, po želji obrazi) | Odloženo | Nove vrste medijev potrebujejo lastne modele, metapodatke in odjemalce. Poljubne video knjižnice so že podprte. |
| Podcasti in avdioknjige z zapomnjenim položajem | Odloženo | Nove vrste medijev potrebujejo lastne modele, metapodatke in odjemalce. Poljubne video knjižnice so že podprte. |
| Dokumentarci, stand-up in YouTube arhiv kot posebne vrste knjižnic | Odloženo | Nove vrste medijev potrebujejo lastne modele, metapodatke in odjemalce. Poljubne video knjižnice so že podprte. |
| Več verzij istega filma (1080p, 4K, režiserjeva verzija) z izbiro ob predvajanju | Delno obstoječe | Lokalne datoteke iste provider identitete so dostopne v podrobnostih; kakovost/izdaja še nista normalizirani za namenski izbirnik. |
| Zaznava podvojenih datotek in predlog za združitev | Odloženo | Enaka pot/provider nista dokaz enake datoteke. Potrebuje stroškovno omejene hash/signature primerjave in varen pregled pred združevanjem. |
| Zaznava poškodovanih datotek (ffprobe napaka) z opozorilom | Delno obstoječe | FFprobe ob predvajanju vrne uporabno napako. Celotni periodični pregledi velikih medijev bi zahtevali čakalno vrsto. |
| Uvoz in izvoz .nfo datotek (Kodi/Emby) | Odloženo | Potrebuje Kodi/Emby združljiv parser in pravila precedence; izvoz ne sme nenamerno spreminjati medijskih map. |
| Zajem slik: plakat, ozadje, logo, sličica epizode | Odloženo | Potrebuje varen upload/storage in pravice. Provider plakati in predogledi že delujejo. |
| Ročno nalaganje lastnih plakatov in ozadij | Odloženo | Potrebuje varen upload/storage in pravice. Provider plakati in predogledi že delujejo. |
| Urejanje metapodatkov: naslov, opis, žanri, oznake | Delno obstoječe | Ročna identifikacija in zaklep provider povezave sta že podprta; prosto urejanje polj/oznak še ne. |
| Podnapisi, plakati in nfo v mapi ob datoteki kot prednostni vir | Odloženo | Potrebuje Kodi/Emby združljiv parser in pravila precedence; izvoz ne sme nenamerno spreminjati medijskih map. |
| Spremljanje map v živo (inotify / FSEvents / ReadDirectoryChanges) namesto ročnega skeniranja | Odloženo | Za omrežne in odstranljive diske je potreben enoten scheduler z debounce/backoff; trenutno sta preverjena ročni inkrementalni in polni pregled. |
| Načrtovani samodejni pregledi (npr. vsako noč ob 3:00) | Odloženo | Za omrežne in odstranljive diske je potreben enoten scheduler z debounce/backoff; trenutno sta preverjena ročni inkrementalni in polni pregled. |
| Samodejno zaznavanje novo priklopljenega diska | Odloženo | Za omrežne in odstranljive diske je potreben enoten scheduler z debounce/backoff; trenutno sta preverjena ročni inkrementalni in polni pregled. |
| Poročilo o neprepoznanih datotekah ("Needs attention") | Uvedeno | Administratorska diagnostika pokaže do 100 datotek za pregled z razlogi. |
| Poraba prostora po knjižnici, najdaljši in največji filmi | Delno uvedeno | Diagnostika pokaže skupno velikost in 10 največjih datotek. Razčlenitev po knjižnici in najdaljši filmi še ne. |
| Seznam manjkajočih epizod za vse serije na enem mestu (Wanted) | Delno obstoječe | Sezone/katalog posamezne serije že označujejo manjkajoče epizode; globalni Wanted in izvoz še nista uvedena. |
| Zaznava nižje kakovosti (npr. 480p, CAM) z opozorilom za nadgradnjo | Odloženo | Potrebuje prebran tehnični indeks vseh datotek; imena CAM/480p sama niso zanesljiv dokaz kakovosti. |

## Podnapisi

| Predlog | Presoja | Izvedba / razlog |
| --- | --- | --- |
| Prenos podnapisov z OpenSubtitles neposredno v mapo ob datoteki | Delno obstoječe | Iskanje in lokalne sledi obstajajo. Prenos zahteva provider download/login pravice in izrecno politiko zapisovanja ob datoteki. |
| Samodejni prenos manjkajočih podnapisov v želenem jeziku | Odloženo | Potrebuje podprt provider API, preverjanje ujemanja in potrjeno zapisovalno politiko medijskih map. |
| Podpora za Podnapisi.net (priročno za slovenske podnapise) | Odloženo | Potrebuje podprt provider API, preverjanje ujemanja in potrjeno zapisovalno politiko medijskih map. |
| Ocena ujemanja podnapisov glede na hash datoteke | Odloženo | Potrebuje podprt provider API, preverjanje ujemanja in potrjeno zapisovalno politiko medijskih map. |
| Samodejna sinhronizacija podnapisov (npr. ffsubsync ali alass) | Odloženo | Zapisano v TODO.md z release matchingom, oceno zamika/drifta, predogledom in undo. ffsubsync/alass nista del trenutnega runtime; potrebna sta čakalna vrsta in meritev ujemanja brez spreminjanja izvirnikov. |
| Pretvorba SRT/ASS/SSA v WebVTT z ohranjenim stilom | Delno obstoječe | FFmpeg pretvori besedilo v WebVTT; ASS napredni slog/animacije se ne ohranijo v celoti. |
| Podpora za ASS animacije (npr. JASSUB) za anime | Odloženo | Nov adapter, identitete, cache, naročnine in pravila razreševanja nasprotujočih se rezultatov so samostojen razvojni sklop. |
| Ročno nalaganje podnapisov med predvajanjem | Odloženo | Potrebuje omejen upload, validacijo, profilne pravice in ločen storage. |
| Lokalno generiranje podnapisov z Whisper, če jih ni | Odloženo | Potrebuje velike lokalne modele, upravljanje virov in preverjanje kakovosti prevodov; ne dodajamo skritih prenosov modelov. |
| Prevod podnapisov v slovenščino z lokalnim modelom | Odloženo | Potrebuje velike lokalne modele, upravljanje virov in preverjanje kakovosti prevodov; ne dodajamo skritih prenosov modelov. |

## Uporabniki, profili in varnost

| Predlog | Presoja | Izvedba / razlog |
| --- | --- | --- |
| Avatarji profilov in barve (izbira iz galerije) | Delno obstoječe | Profili imajo obarvane inicialke; galerija/upload še nista uvedena. |
| Otroški profil z omejeno knjižnico in brez nastavitev | Odloženo | Varnostno celovit sklop: potrebuje zanesljive starostne oznake ter strežniško omejitev vseh seznamov, metapodatkov, tokov in profilnih prehodov. Skrivanje UI bi ustvarilo lažen občutek zaščite. |
| Časovna omejitev ogleda (npr. 2 uri/dan za otroka) | Odloženo | Potrebuje dnevni dnevnik porabe in strežniško omejitev predvajanja, ne samo števec v brskalniku. |
| Dnevnik dejavnosti po uporabniku (kdo je kaj gledal) | Obstoječe | Administrator vidi aktivne in nedavne profile/predvajanja; to ni nespremenljiv revizijski dnevnik. |
| Dvofaktorska avtentikacija (TOTP) | Odloženo | Avtentikacijski razvojni sklop s shranjevanjem/verifikacijo poverilnic, potekom/preklicem in obnovitvijo. Ne uvajamo nepreverjenega varnostnega mehanizma. |
| Quick Connect: prijava na TV s 6-mestno kodo iz telefona | Odloženo | Avtentikacijski razvojni sklop s shranjevanjem/verifikacijo poverilnic, potekom/preklicem in obnovitvijo. Ne uvajamo nepreverjenega varnostnega mehanizma. |
| Vabila za nove uporabnike s povezavo | Odloženo | Avtentikacijski razvojni sklop s shranjevanjem/verifikacijo poverilnic, potekom/preklicem in obnovitvijo. Ne uvajamo nepreverjenega varnostnega mehanizma. |
| Dovoljenja po knjižnici: kdo vidi katero | Odloženo | Varnostno celovit sklop: potrebuje zanesljive starostne oznake ter strežniško omejitev vseh seznamov, metapodatkov, tokov in profilnih prehodov. Skrivanje UI bi ustvarilo lažen občutek zaščite. |
| Omejitev števila hkratnih tokov po uporabniku | Delno uvedeno | HLS je globalno omejen na štiri seje, s profilnim lastništvom. Čakalna vrsta in kvote po uporabniku še niso uvedene. |
| Omejitev bitrate pri oddaljenem dostopu | Delno obstoječe | Ročna kakovost in ABR omogočata nižji bitrate. Strežniška klasifikacija lokalno/oddaljeno in trda kvota še nista uvedeni. |
| SSO/OIDC (Authelia, Authentik, Google) in LDAP | Odloženo | Avtentikacijski razvojni sklop s shranjevanjem/verifikacijo poverilnic, potekom/preklicem in obnovitvijo. Ne uvajamo nepreverjenega varnostnega mehanizma. |
| API ključi za integracije | Odloženo | Avtentikacijski razvojni sklop s shranjevanjem/verifikacijo poverilnic, potekom/preklicem in obnovitvijo. Ne uvajamo nepreverjenega varnostnega mehanizma. |
| Seznam aktivnih sej z odjavo drugih naprav | Uvedeno | Profil vidi prijavne seje svojega računa in lahko odjavi druge naprave. |
| Prijava s ključi dostopa (WebAuthn / passkeys) | Odloženo | Avtentikacijski razvojni sklop s shranjevanjem/verifikacijo poverilnic, potekom/preklicem in obnovitvijo. Ne uvajamo nepreverjenega varnostnega mehanizma. |
| Zaščita pred brute-force na nivoju IP, ne samo računa | Delno obstoječe | Računska in PIN omejitev obstajata. IP omejitev potrebuje pravilno konfigurirano zaupanje proxyjem. |
| Revizijski dnevnik (audit log) administratorja | Odloženo | Potrebuje definirane dogodke, omejeno hrambo in celovitost; trenutni scan/activity pogled ni audit log. |
| Samodejna varnostna kopija baze SQLite z obnovitvijo iz vmesnika | Delno dokumentirano | Navodila za backup/restore z ustavljenim strežnikom. Samodejni online backup in UI restore zahtevata varen načrt za WAL in rollback. |

## Socialne funkcije

| Predlog | Presoja | Izvedba / razlog |
| --- | --- | --- |
| Watch Together / SyncPlay: sinhronizirano gledanje z več uporabniki in klepetom | Odloženo | Samostojen socialni sklop: novi dogodki/pravice in trajna zgodovina. Zunanji kanali potrebujejo račune in nastavitve; profilni watchlist že obstaja. |
| Priporočila med uporabniki ("Anja je gledala to") | Odloženo | Samostojen socialni sklop: novi dogodki/pravice in trajna zgodovina. Zunanji kanali potrebujejo račune in nastavitve; profilni watchlist že obstaja. |
| Ocene (zvezdice ali všeček) po profilu | Odloženo | Samostojen socialni sklop: novi dogodki/pravice in trajna zgodovina. Zunanji kanali potrebujejo račune in nastavitve; profilni watchlist že obstaja. |
| Zahteve za vsebino ("Prosim dodaj ta film"), ki jih vidi administrator | Odloženo | Samostojen socialni sklop: novi dogodki/pravice in trajna zgodovina. Zunanji kanali potrebujejo račune in nastavitve; profilni watchlist že obstaja. |
| Skupni seznami za ogled (npr. družinski) | Odloženo | Samostojen socialni sklop: novi dogodki/pravice in trajna zgodovina. Zunanji kanali potrebujejo račune in nastavitve; profilni watchlist že obstaja. |
| Obvestila: nova epizoda, nov film, zahteva izpolnjena (push, e-pošta, Discord, Telegram) | Odloženo | Samostojen socialni sklop: novi dogodki/pravice in trajna zgodovina. Zunanji kanali potrebujejo račune in nastavitve; profilni watchlist že obstaja. |
| Zasebne beležke k filmu | Odloženo | Samostojen socialni sklop: novi dogodki/pravice in trajna zgodovina. Zunanji kanali potrebujejo račune in nastavitve; profilni watchlist že obstaja. |
| Letni povzetek ("Tvoje leto v filmih", kot Spotify Wrapped) | Odloženo | Samostojen socialni sklop: novi dogodki/pravice in trajna zgodovina. Zunanji kanali potrebujejo račune in nastavitve; profilni watchlist že obstaja. |

## Integracije in avtomatizacija

| Predlog | Presoja | Izvedba / razlog |
| --- | --- | --- |
| Trakt sinhronizacija (scrobble, watchlist, ocene) | Odloženo | Potrebuje dokumentirano integracijsko pogodbo, avtorizacijo, migracije in preizkuse proti ciljnemu sistemu. Ne izpostavljamo medijskih URL-jev brez avtorizacije. |
| Letterboxd izvoz in uvoz | Odloženo | Potrebuje dokumentirano integracijsko pogodbo, avtorizacijo, migracije in preizkuse proti ciljnemu sistemu. Ne izpostavljamo medijskih URL-jev brez avtorizacije. |
| Sonarr / Radarr / Prowlarr in Overseerr / Jellyseerr povezave | Odloženo | Potrebuje dokumentirano integracijsko pogodbo, avtorizacijo, migracije in preizkuse proti ciljnemu sistemu. Ne izpostavljamo medijskih URL-jev brez avtorizacije. |
| Webhooki ob dogodkih (začetek predvajanja, nov element, napaka) | Odloženo | Potrebuje dokumentirano integracijsko pogodbo, avtorizacijo, migracije in preizkuse proti ciljnemu sistemu. Ne izpostavljamo medijskih URL-jev brez avtorizacije. |
| Home Assistant integracija (npr. zatemni luči ob predvajanju) | Odloženo | Potrebuje dokumentirano integracijsko pogodbo, avtorizacijo, migracije in preizkuse proti ciljnemu sistemu. Ne izpostavljamo medijskih URL-jev brez avtorizacije. |
| Discord Rich Presence | Odloženo | Potrebuje dokumentirano integracijsko pogodbo, avtorizacijo, migracije in preizkuse proti ciljnemu sistemu. Ne izpostavljamo medijskih URL-jev brez avtorizacije. |
| Statistika v slogu Tautulli (največ gledano, ure, naprave) | Odloženo | Potrebuje dokumentirano integracijsko pogodbo, avtorizacijo, migracije in preizkuse proti ciljnemu sistemu. Ne izpostavljamo medijskih URL-jev brez avtorizacije. |
| Prometheus/Grafana metrike (CPU, tokovi, transkodiranje) | Odloženo | Potrebuje dokumentirano integracijsko pogodbo, avtorizacijo, migracije in preizkuse proti ciljnemu sistemu. Ne izpostavljamo medijskih URL-jev brez avtorizacije. |
| OpenAPI/Swagger dokumentacija za API | Odloženo | Potrebuje dokumentirano integracijsko pogodbo, avtorizacijo, migracije in preizkuse proti ciljnemu sistemu. Ne izpostavljamo medijskih URL-jev brez avtorizacije. |
| Plugin sistem ali skriptna kavlja (hooks) | Odloženo | Potrebuje dokumentirano integracijsko pogodbo, avtorizacijo, migracije in preizkuse proti ciljnemu sistemu. Ne izpostavljamo medijskih URL-jev brez avtorizacije. |
| Zunanji predvajalniki: odpiranje v VLC/mpv, izvoz M3U za IPTV aplikacije | Odloženo | Potrebuje dokumentirano integracijsko pogodbo, avtorizacijo, migracije in preizkuse proti ciljnemu sistemu. Ne izpostavljamo medijskih URL-jev brez avtorizacije. |
| Delna kompatibilnost API-ja z Jellyfinom, ki bi odprla veliko obstoječih aplikacij | Odloženo | Potrebuje dokumentirano integracijsko pogodbo, avtorizacijo, migracije in preizkuse proti ciljnemu sistemu. Ne izpostavljamo medijskih URL-jev brez avtorizacije. |

## Pretvorba in zmogljivost

| Predlog | Presoja | Izvedba / razlog |
| --- | --- | --- |
| Predpomnilnik pretvorb (isti film večkrat ne transkodiraj) | Odloženo | Pretvorbe so vezane na uporabnikovo sled/seek/kakovost. Trajni cache potrebuje kvote, invalidacijo in življenjski cikel opravil. |
| Predhodna pretvorba ("Optimized versions") ponoči | Odloženo | Pretvorbe so vezane na uporabnikovo sled/seek/kakovost. Trajni cache potrebuje kvote, invalidacijo in življenjski cikel opravil. |
| Omejitev števila hkratnih transkodiranj in čakalna vrsta | Delno uvedeno | HLS je globalno omejen na štiri seje, s profilnim lastništvom. Čakalna vrsta in kvote po uporabniku še niso uvedene. |
| Zaznava strojne pospešitve: NVENC, QSV, VAAPI, VideoToolbox, AMF z avtomatskim testom | Delno obstoječe | Izpis in smoke-test VideoToolbox/NVENC/QSV z libx264 fallbackom obstajata. VAAPI/AMF zahtevata fizično ciljno okolje. |
| Direct Stream: samo sprememba zabojnika (MKV → fMP4), brez ponovnega kodiranja videa | Odloženo | Potrebuje capability negotiation in ločen remux načrt; sedanja preverjena skupna ciljna formata sta H.264/AAC in izvirni združljivi MP4. |
| HEVC/AV1 direct play, kjer ga brskalnik podpira | Odloženo | Potrebuje capability negotiation in ločen remux načrt; sedanja preverjena skupna ciljna formata sta H.264/AAC in izvirni združljivi MP4. |
| 4K s HEVC, kjer je odjemalec sposoben | Odloženo | Potrebuje capability negotiation in ločen remux načrt; sedanja preverjena skupna ciljna formata sta H.264/AAC in izvirni združljivi MP4. |
| Preverjanje združljivosti brskalnika (MediaCapabilities API) pred izbiro načina | Odloženo | Potrebuje capability negotiation in ločen remux načrt; sedanja preverjena skupna ciljna formata sta H.264/AAC in izvirni združljivi MP4. |
| Pametno predpomnjenje sličic in plakatov (WebP/AVIF, različne velikosti) | Delno uvedeno | Plakati so lazy-loaded; novi JPEG predogledi imajo omejen strežniški cache. WebP/AVIF pipeline še ni. |
| Lazy loading in virtualiziran seznam za knjižnice z več tisoč naslovi | Delno obstoječe | Plakati se nalagajo leno; virtualizacija celotnega seznama zahteva meritev in spremembo fokusa/navigacije. |
| Indeks za hitro iskanje (SQLite FTS5) | Odloženo | Najprej je potreben normaliziran indeks vsebin in paginiran API; obstoječe iskanje se izvaja v odjemalcu. |
| Merjenje hitrosti diska ob prvem predvajanju z nasvetom | Odloženo | Samodejno branje velikih datotek ob predvajanju bi tekmovalo s tokom; potreben je izrecen diagnostični preizkus. |

## Prenos in offline

| Predlog | Presoja | Izvedba / razlog |
| --- | --- | --- |
| Prenos datoteke za offline (z dovoljenjem) | Odloženo | Potrebuje dovoljenja, kvote, varno shrambo na napravi in usklajevanje verzij. PWA sama ne pomeni offline videov. |
| Mobile Sync: predpripravljena manjša verzija za potovanja | Odloženo | Potrebuje dovoljenja, kvote, varno shrambo na napravi in usklajevanje verzij. PWA sama ne pomeni offline videov. |
| Izvoz seznama s sinhronizacijo za posamezne naprave | Odloženo | Potrebuje dovoljenja, kvote, varno shrambo na napravi in usklajevanje verzij. PWA sama ne pomeni offline videov. |

## Vmesnik in dostopnost

| Predlog | Presoja | Izvedba / razlog |
| --- | --- | --- |
| Svetla in temna tema ter samodejna po sistemu | Odloženo | Barve so še delno zapisane neposredno v CSS; treba jih je najprej sistematično pretvoriti v semantične tokene. |
| Slovenščina, angleščina in druge lokalizacije (i18n) | Odloženo | Vmesnik ima veliko inline HTML/JS besedil. Potrebna je centralizacija vseh nizov in celovita lokalizacija. |
| TV način z navigacijo s smernimi tipkami in daljincem (10-foot UI) | Delno uvedeno | Kartice so fokusabilne in podpirajo Enter/Space. Polna prostorska navigacija in 10-foot postavitev zahtevata preizkus na TV. |
| Dostopnost: ARIA oznake, fokus, kontrast, bralniki zaslona | Delno uvedeno | Oznake kontrol, fokus, tipkovnica, statusna sporočila in reduced motion; to ni popoln WCAG audit. |
| Zvočni opisi (audio description), če so na voljo kot sled | Delno uvedeno | Izbirnik pokaže jezik/naslov/kanale vseh zvočnih sledi, vključno z audio description, če je tako označena. |
| Hover kartice s kratkim opisom | Odloženo | Najprej so prednostni stabilni podatki in preizkušeno predvajanje. Obstoječi load/error statusi ostajajo. |
| Skeleton nalaganje namesto praznih strani | Odloženo | Najprej so prednostni stabilni podatki in preizkušeno predvajanje. Obstoječi load/error statusi ostajajo. |
| Odjemalci za Android TV, Apple TV, Fire TV, LG/Samsung (ali vsaj paket Capacitor/Tauri) | Odloženo | Samostojni native projekti potrebujejo pakiranje, podpisovanje, posodobitve in naprave. Namestljiva spletna aplikacija je uvedena. |
| Namizna aplikacija (Tauri) z vgrajenim mpv za direct play brez omejitev brskalnika | Odloženo | Samostojni native projekti potrebujejo pakiranje, podpisovanje, posodobitve in naprave. Namestljiva spletna aplikacija je uvedena. |
| Barve kartice iz plakata (ColorThief) | Odloženo | Dekorativno; pridobivanje barv iz oddaljenih plakatov potrebuje varen CORS/cache pipeline. |
| Ob zagonu zaznava, ali je strežnik dosegljiv, z jasnim opisom napake | Uvedeno | Stabilno startup opozorilo z retry in PWA offline zaslon; brez neskončnega reload cikla. |

## Administracija in vzdrževanje

| Predlog | Presoja | Izvedba / razlog |
| --- | --- | --- |
| Nadzorna plošča: CPU, RAM, disk, aktivni tokovi, FFmpeg procesi | Delno obstoječe | Aktivni tokovi in profilna dejavnost; dodani health/library podatki. CPU/RAM/FFmpeg sistemske metrike še niso. |
| Pregled dnevnikov v vmesniku z iskanjem in filtri | Delno obstoječe | Scan dnevnik in server-health diagnostika; globalni strežniški log viewer z iskanjem še ni. |
| Preverjanje zdravja: manjkajoči ffmpeg, token TMDB, dostopnost map | Delno uvedeno | Oznake kontrol, fokus, tipkovnica, statusna sporočila in reduced motion; to ni popoln WCAG audit. |
| Samodejne posodobitve ali obvestilo o novi različici | Odloženo | Potrebuje verzionirane izdaje, podpisovanje in rollback. Docker ima dokumentiran ročni update. |
| Docker slika in primer docker-compose | Delno uvedeno | Dockerfile in Compose z neprivilegiranim uporabnikom, read-only mediji in persistentnim data volume. Compose validacija uspe; build/run slike ni preverjen zaradi nedosegljivega Docker daemona. Javna registry slika ni objavljena. |
| Namestitveni čarovnik ob prvem zagonu (TMDB ključ, knjižnice, jezik) | Delno obstoječe | Prvi zagon ustvari ownerja; ključi in knjižnice se nastavijo v Settings. Večkorakni onboarding še ni. |
| Servis (systemd / launchd / Windows service) z navodili | Delno dokumentirano | Native service navodila in systemd primer; avtomatski installer za vse OS še ni. |
| Uvoz zgodovine iz Plexa, Jellyfina in Trakta | Odloženo | Potrebuje dokumentirano integracijsko pogodbo, avtorizacijo, migracije in preizkuse proti ciljnemu sistemu. Ne izpostavljamo medijskih URL-jev brez avtorizacije. |
| Izvoz in uvoz nastavitev | Odloženo | Potrebuje verzioniran format, validacijo provider identitet in pravila združevanja brez prepisovanja uporabnikov. |
| Tihi način skeniranja z nizko prioriteto za počasne diske | Odloženo | Potrebuje platformni I/O scheduler in meritve. Ročni cancel/incremental že obstajata. |

## Posebnosti, ki ločijo PlexV2 od Plexa

| Predlog | Presoja | Izvedba / razlog |
| --- | --- | --- |
| "Prej v seriji": povzetek prejšnje epizode brez spoilerjev, lokalno ali prek API-ja | Odloženo | Potrebuje licenciran/pridobljen vir povzetkov in preverljivo zaščito pred spoilerji; ne izmišljamo vsebine epizod. |
| Zaznava duplikatov med knjižnicami z vizualno primerjavo kakovosti | Odloženo | Enaka pot/provider nista dokaz enake datoteke. Potrebuje stroškovno omejene hash/signature primerjave in varen pregled pred združevanjem. |
| Spoiler zaščita: skrij opise in sličice nepogledanih epizod | Odloženo | Potrebuje enoten status ogledanosti v katalogih, opisih, priporočilih in slikah; zgolj CSS skrivanje ne zadošča. |
| Statistika "Koliko ur si gledal" po žanru, mesecu in serijah | Odloženo | Zadnji položaj ni število dejansko ogledanih ur; potrebna je trajna dogodkovna zgodovina. |
| Slovenski filmi: posebna polica z lokalnimi deli in oznakami RTV/SFC | Odloženo | Potrebuje preverjene države/jezike in RTV/SFC identitete; naslov ali uporabnikov jezik nista dokaz izvora. |
| Samodejni popis manjkajočih sezon z izvozom seznama za nakup ali iskanje | Delno obstoječe | Sezone/katalog posamezne serije že označujejo manjkajoče epizode; globalni Wanted in izvoz še nista uvedena. |

## Predlagan vrstni red izvedbe

| Predlog | Presoja | Izvedba / razlog |
| --- | --- | --- |
| Kratkoročno (nekaj dni): zvočne sledi, mark as watched, bližnjice na tipkovnici, naslednja epizoda s kartico, filtri in razvrščanje. | Presojeno | Prioritete so priporočilo iz dokumenta. Izvedeni sklopi in odložene integracije so navedeni posamično zgoraj. |
| Srednjeročno: skip intro, trickplay, starševski nadzor, adaptive bitrate, burn-in podnapisov, Trakt, obvestila. | Presojeno | Prioritete so priporočilo iz dokumenta. Izvedeni sklopi in odložene integracije so navedeni posamično zgoraj. |
| Dolgoročno: HDR tone mapping, Watch Together, aplikacije za TV, glasba in fotografije, plugin sistem, kompatibilnost z Jellyfin API-jem. | Presojeno | Prioritete so priporočilo iz dokumenta. Izvedeni sklopi in odložene integracije so navedeni posamično zgoraj. |
