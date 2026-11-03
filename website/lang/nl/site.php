<?php

return [
    'meta_title' => '2DriveScan — Gratis schijfruimte-analyse voor macOS, Windows en Linux',
    'meta_description' => 'Zie wat je schijfruimte opslokt. 2DriveScan scant schijven in seconden en toont elke map als interactieve sunburst. Gratis en open source voor macOS, Windows en Linux.',
    'skip' => 'Naar de inhoud',

    'nav' => [
        'features' => 'Functies',
        'demo' => 'Live demo',
        'download' => 'Downloaden',
        'changelog' => 'Wijzigingen',
        'github' => 'GitHub',
        'menu' => 'Menu',
        'language' => 'Taal',
    ],

    'os' => [
        'macos' => 'macOS',
        'windows' => 'Windows',
        'linux' => 'Linux',
    ],

    'hero' => [
        'eyebrow' => 'Gratis & open source · macOS · Windows · Linux',
        'title' => 'Zie wat je schijf vult.',
        'title_highlight' => 'En maak weer ruimte.',
        'lead' => '2DriveScan scant je schijven in seconden en maakt van elke map een interactieve sunburst. Vind de ruimtevreters, verzamel ze en verplaats ze naar de prullenmand. Geen account, geen reclame, geen kosten.',
        'download_for' => 'Download voor :os',
        'other_platforms' => 'Andere platformen',
        'try_demo' => 'Probeer het in je browser',
        'version_line' => 'Versie :version · :size',
        'unavailable' => 'Download de nieuwste versie via GitHub',
        'demo_caption' => 'Dit is de echte app, met voorbeelddata in je browser. Klik op een ring om in te zoomen en op het midden om terug te gaan.',
        'open_fullscreen' => 'Open de demo schermvullend',
    ],

    'strip' => [
        'rust' => 'Gebouwd in Rust',
        'tiny' => 'Kleine installers',
        'private' => 'Geen account, geen tracking',
        'offline' => 'Werkt offline',
        'open' => 'MIT-licentie',
    ],

    'features' => [
        'title' => 'Alles om weer ruimte te maken',
        'lead' => 'Snel waar het telt, mooi in alles.',
        'items' => [
            'sunburst' => ['Een interactieve kaart van je schijf', 'Elke ring is een mapniveau. Wijs aan om te bekijken, klik om erin te duiken en zie de grafiek soepel naar het volgende niveau glijden.'],
            'speed' => ['Snel. Echt snel.', 'Een parallelle scanner in Rust leest honderdduizenden bestanden per seconde, telt de ruimte die bestanden echt op schijf innemen en downloadt nooit bestanden die alleen in iCloud of andere cloudmappen staan.'],
            'hidden' => ['Verborgen ruimte, uitgelegd', 'Snapshots, swap en systeembestanden die bij geen enkele map horen, krijgen een eigen segment. Zo kloppen de getallen eindelijk.'],
            'largest' => ['Vind de grote jongens', 'Mappen gesorteerd op grootte, de grootste bestanden onder de huidige map, kleuren per bestandstype en direct zoeken.'],
            'saved' => ['Scans worden bewaard', 'Open je laatste scan meteen opnieuw. Een nieuwe scan leest alleen de mappen die veranderd zijn, en een onderbroken scan gaat verder waar hij gebleven was.'],
            'collector' => ['Veilig opruimen', 'Sleep items naar de verzamelbak, controleer ze en verplaats ze in één keer naar de prullenmand. Er wordt nooit iets definitief verwijderd.'],
            'native' => ['Thuis op elk platform', 'macOS op Apple Silicon en Intel, Windows op x64 en ARM, en alle grote Linux-distributies. In licht en donker.'],
            'private' => ['Privacy vanaf de basis', 'Je bestanden verlaten nooit je computer. Geen account, geen telemetrie, geen internetverbinding nodig.'],
        ],
    ],

    'how' => [
        'title' => 'In drie stappen een opgeruimde schijf',
        'steps' => [
            ['Kies een schijf of map', 'Kies je systeemschijf, een externe schijf of een willekeurige map. Het scannen begint meteen.'],
            ['Verken', 'Zoom door de sunburst, zoek, en sorteer op grootte of bestandstype om te vinden wat je niet meer nodig hebt.'],
            ['Ruim op', 'Verzamel bestanden en mappen, bekijk het totaal en verplaats ze naar de prullenmand. Terugzetten kan zolang je die niet leegt.'],
        ],
    ],

    'showcase' => [
        'title' => 'Ziet er net zo goed uit als het werkt',
        'lead' => 'Ontworpen voor licht en donker, met kleuren die laten zien waar je ruimte gebleven is.',
        'dark' => 'Kleuren per map in donkere modus',
        'light' => 'Kleuren per bestandstype in lichte modus',
    ],

    'download' => [
        'title' => '2DriveScan downloaden',
        'lead' => 'Gratis voor iedereen. Kies je platform.',
        'page_lead' => 'Versie :version, uitgebracht op :date.',
        'page_lead_nodate' => 'Versie :version.',
        'macos_req' => 'macOS 10.15 of nieuwer · Apple Silicon & Intel',
        'windows_req' => 'Windows 10 en 11 · x64 en ARM64',
        'linux_req' => 'Ubuntu, Debian, Fedora, openSUSE, Arch en meer',
        'all' => 'Alle downloads en instructies',
        'installer' => 'Installer',
        'recommended' => 'Aanbevolen',
        'checksum' => 'SHA-256',
        'unavailable' => 'Release-informatie is tijdelijk niet beschikbaar. Je kunt de installers altijd via GitHub downloaden.',
        'on_github' => 'Bekijk deze release op GitHub',
        'install_title' => 'Installeren',
        'macos_steps' => [
            'Open het .dmg-bestand en sleep 2DriveScan naar je map Apps.',
            'De app is ondertekend en genotariseerd door Apple en opent zonder waarschuwingen.',
            'Geef voor een volledige scan Volledige schijftoegang in Systeeminstellingen → Privacy en beveiliging.',
        ],
        'windows_steps' => [
            'Start het setup-bestand. Beheerdersrechten zijn niet nodig.',
            'Windows SmartScreen kan waarschuwen voor een nieuwe app. Kies “Meer informatie” en daarna “Toch uitvoeren”.',
            'Liever een MSI-pakket? Gebruik dan de .msi voor x64.',
        ],
        'linux_steps' => [
            'Debian, Ubuntu, Mint, Pop!_OS: installeer het .deb-pakket.',
            'Fedora, openSUSE, RHEL: installeer het .rpm-pakket.',
            'Elke andere distributie, ook Arch: maak de AppImage uitvoerbaar en start hem.',
        ],
        'source_title' => 'Zelf bouwen',
        'source_text' => '2DriveScan is open source. In de README op GitHub staat hoe je de app zelf bouwt.',
    ],

    'faq' => [
        'title' => 'Vragen & antwoorden',
        'items' => [
            ['Is 2DriveScan echt gratis?', 'Ja. 2DriveScan is open source onder de MIT-licentie. Er is geen betaalde versie, geen reclame en geen account.'],
            ['Is het een alternatief voor DaisyDisk?', 'Ja. 2DriveScan geeft je hetzelfde soort interactieve schijfkaart, en werkt ook op Windows en Linux.'],
            ['Kan ik per ongeluk iets belangrijks verwijderen?', 'Nee. 2DriveScan verwijdert nooit uit zichzelf. Items die je verzamelt gaan pas na jouw bevestiging naar de prullenmand of prullenbak, dus je kunt ze altijd terugzetten.'],
            ['Waarom vraagt macOS om volledige schijftoegang?', 'macOS beschermt sommige mappen, zoals de gegevens van Mail en Safari. Zonder toegang telt 2DriveScan die als verborgen ruimte. Geef volledige schijftoegang in Systeeminstellingen → Privacy en beveiliging voor het volledige beeld.'],
            ['Moet ik elke keer op een volledige scan wachten?', 'Nee. Elke scan wordt bewaard, zodat je hem meteen weer kunt openen. Een nieuwe scan leest alleen mappen die sinds de vorige keer veranderd zijn; dat duurt meestal seconden. Stop je een scan of sluit je de app, dan gaat de volgende scan verder waar hij gebleven was.'],
            ['Wat is “verborgen ruimte”?', 'Ruimte die op een schijf in gebruik is, maar niet bij een zichtbaar bestand hoort: lokale snapshots, swap, systeembestanden en mappen die je niet mag lezen.'],
            ['Stuurt 2DriveScan gegevens door?', 'Nee. Scans gebeuren volledig op je eigen computer en de app heeft geen internetverbinding nodig.'],
            ['Welke Linux-distributies worden ondersteund?', 'Elke recente distributie. Gebruik de .deb op Debian- en Ubuntu-systemen, de .rpm op Fedora en openSUSE, en de AppImage overal anders, ook op Arch.'],
        ],
    ],

    'changelog' => [
        'title' => 'Wijzigingen',
        'lead' => 'Wat er in elke versie van 2DriveScan veranderd is.',
        'empty' => 'Nog geen wijzigingen vastgelegd.',
        'latest' => 'Nieuwste',
    ],

    'footer' => [
        'tagline' => 'De gratis, open source schijfruimte-analyse.',
        'made' => 'Gemaakt met Rust en Tauri. Uitgebracht onder de MIT-licentie.',
        'product' => 'Product',
        'project' => 'Project',
        'source' => 'Broncode',
        'releases' => 'Alle releases',
        'issues' => 'Probleem melden',
        'license' => 'Licentie',
    ],
];
