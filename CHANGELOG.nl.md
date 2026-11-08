# Wijzigingen

Alle belangrijke wijzigingen in 2DriveScan. De Engelse versie staat in `CHANGELOG.md`.

## 0.5.0 – 2026-11-08

- **Wat er veranderd is sinds de vorige scan.** 2DriveScan bewaart de vorige scan van elke schijf of map en
  vergelijkt: het nieuwe tabblad **Veranderingen** toont wat er gegroeid, gekrompen of nieuw is, en
  **Veranderingen** kleurt de grafiek daarnaar (rood gegroeid, turquoise gekrompen, roze nieuw). Ook de tooltips
  tonen de verandering.
- **Dubbele bestanden.** Het tabblad **Duplicaten** zoekt bestanden met precies dezelfde inhoud onder de open map:
  eerst op grootte, dan op begin en eind, dan op de hele inhoud. Ook hernoemde kopieën worden gevonden; harde
  links tellen niet als kopie. Verzamel de extra kopieën van één groep of van alle groepen; het origineel blijft.
- **Opruimsuggesties.** Het tabblad **Opruimen** toont plekken waar ruimte blijft hangen: app-caches,
  logbestanden, Xcode-builddata en -archieven, simulatorcaches, back-ups van iPhone en iPad, caches van
  npm/Gradle/Cargo, `node_modules`-mappen, downloads die 3 maanden niet zijn gewijzigd, en de prullenmand. Met één
  klik in de verzamelbak; er gaat niets weg zonder jouw bevestiging.
- **Treemap.** Wissel de grafiek tussen de sunburst en een treemap (geneste vakken), naast de kleurknoppen of in
  de instellingen.
- **Leeftijdsfilter.** De grootste bestanden kun je beperken tot bestanden die 3 maanden, 6 maanden, 1 of 2 jaar
  niet zijn gewijzigd, en ze tonen wanneer ze voor het laatst zijn gewijzigd.
- **Mappen uitsluiten.** Klik met rechts op een map en kies **Uitsluiten van scans**, of beheer de lijst in
  Instellingen › Scans. Bij een hele schijf telt uitgesloten ruimte als verborgen ruimte.
- **Exporteren als CSV.** De nieuwe knop naast de mapnaam bewaart de mappen (en bestanden vanaf 1 MB) met
  groottes en datums als CSV-bestand.

Bewaarde scans bevatten nu bestandsdatums; de eerste scan na de update leest daarom alles één keer opnieuw.

## 0.4.0 – 2026-11-04

- **Instellingen.** Een nieuw instellingenvenster (het tandwiel in de titelbalk, of Ctrl/⌘ + ,) met:
  - **Updates:** zie welke versie je hebt, controleer op een nieuwe, installeer die met één klik en zet het
    controleren bij het opstarten aan of uit.
  - **Thema** (systeem, licht, donker) en **taal** (systeem, English, Nederlands), direct toegepast.
  - **Groottes** in eenheden van 1000 of 1024, of automatisch zoals je systeem.
  - **Grafiek:** kleuren per map of per bestandstype (wordt onthouden), 4 tot 7 mapniveaus, en animaties aan
    of uit.
  - **Bewaarde scans:** zie hoeveel ruimte ze innemen en verwijder ze.
  - **Over:** links naar de website, GitHub, de wijzigingen en de formulieren voor problemen en ideeën, en de
    sneltoetsen.
- Het tandwiel krijgt een stip als er een nieuwe versie is.
- Opgelost: primaire knoppen verloren hun kleur als je erover bewoog, waardoor de tekst in licht thema
  slecht leesbaar was.

## 0.3.0 – 2026-11-04

- **Automatische updates.** 2DriveScan kijkt bij het opstarten of er een nieuwe versie is en installeert die met
  één klik ("Installeren en herstarten"), met een voortgangsbalk. Updates zijn ondertekend; alles zonder
  geldige handtekening wordt geweigerd. Werkt voor alle installers: macOS, Windows (setup.exe en .msi) en
  Linux (.deb, .rpm, AppImage).
- Het startscherm heeft **Controleren op updates** en een schakelaar om het automatisch controleren uit te zetten.
- Tijdens een scan worden geen updates geïnstalleerd.

Deze versie moet je één keer zelf installeren; daarna werkt 2DriveScan zichzelf bij.

## 0.2.1 – 2026-11-03

- Opgelost: een afgemaakte scan werd als "onafgemaakt" bewaard als sommige mappen niet gelezen konden worden
  (normaal op macOS zonder volledige schijftoegang). De app ging dan terug naar het startscherm in plaats van
  het resultaat te tonen. Gebruik één keer **Scan voortzetten** om hem af te maken; de voortgang die je al
  hebt, blijft bewaard.

## 0.2.0 – 2026-11-03

Veel snellere scans, en je hoeft niet meer bij nul te beginnen.

- **Cloudbestanden worden niet meer gedownload.** Op macOS liet het scannen van iCloud Drive en andere
  cloudmappen het systeem alles downloaden wat alleen in de cloud stond. Daardoor kon een scan een half uur
  duren. Die mappen worden nu overgeslagen; bestanden die niet gedownload zijn, nemen toch geen schijfruimte in.
- **Snellere macOS-scanner** die namen, groottes en datums van veel bestanden per systeemaanroep leest
  (`getattrlistbulk`).
- **Scans worden bewaard.** Het startscherm laat zien wanneer elke schijf het laatst gescand is; open het
  resultaat meteen.
- **Snel opnieuw scannen.** Een nieuwe scan leest alleen de mappen die sinds de vorige scan veranderd zijn en
  neemt de rest over: meestal seconden in plaats van minuten. Houd Alt/Option ingedrukt bij Opnieuw scannen
  voor een volledige scan.
- **Onderbroken scans gaan verder.** Stop een scan of sluit de app, en de volgende scan gaat verder waar hij
  gebleven was.

## 0.1.1 – 2026-11-03

- De macOS-app is nu ondertekend met een Developer ID-certificaat en genotariseerd door Apple. Hij opent
  daardoor zonder beveiligingswaarschuwingen.

## 0.1.0 – 2026-11-03

De eerste publieke versie.

- **Interactieve sunburst** van elke schijf of map, met soepel geanimeerd inzoomen.
- **Snelle parallelle scanner** in Rust die de echte ruimte op schijf telt en harde links correct verwerkt.
- **Verborgen ruimte**: gebruikte ruimte die bij geen enkel bestand hoort, krijgt een eigen segment.
- **Zijpaneel** met mapinhoud, de grootste bestanden, een verdeling per bestandstype en zoeken.
- **Kleur per map of per bestandstype.**
- **Verzamelbak**: sleep items erheen en verplaats ze in één keer naar de prullenmand of prullenbak.
- Licht en donker thema, Engels en Nederlands.
- Installers voor macOS (universal), Windows (x64, ARM64) en Linux (deb, rpm, AppImage voor x64 en ARM64).
