<?php

return [
    'meta_title' => '2DriveScan — Free disk space analyzer for macOS, Windows and Linux',
    'meta_description' => 'See what is taking up your disk space. 2DriveScan scans drives in seconds and shows every folder as an interactive sunburst. Free and open source for macOS, Windows and Linux.',
    'skip' => 'Skip to content',

    'nav' => [
        'features' => 'Features',
        'demo' => 'Live demo',
        'download' => 'Download',
        'changelog' => 'Changelog',
        'github' => 'GitHub',
        'donate' => 'Donate',
        'menu' => 'Menu',
        'language' => 'Language',
    ],

    'os' => [
        'macos' => 'macOS',
        'windows' => 'Windows',
        'linux' => 'Linux',
    ],

    'hero' => [
        'eyebrow' => 'Free & open source · macOS · Windows · Linux',
        'title' => 'See what fills your disk.',
        'title_highlight' => 'Take your space back.',
        'lead' => '2DriveScan scans your drives in seconds and turns every folder into an interactive sunburst. Spot the space hogs, collect them and move them to the Trash. No account, no ads, no cost.',
        'download_for' => 'Download for :os',
        'other_platforms' => 'Other platforms',
        'try_demo' => 'Try it in your browser',
        'version_line' => 'Version :version · :size',
        'unavailable' => 'Get the latest version on GitHub',
        'demo_caption' => 'This is the real app, running in your browser with sample data. Click a ring to zoom in, click the center to go back.',
        'open_fullscreen' => 'Open the demo full screen',
    ],

    'strip' => [
        'rust' => 'Built in Rust',
        'tiny' => 'Tiny installers',
        'private' => 'No account, no tracking',
        'offline' => 'Works offline',
        'open' => 'MIT licensed',
    ],

    'features' => [
        'title' => 'Everything you need to free up space',
        'lead' => 'Fast where it matters, beautiful everywhere else.',
        'items' => [
            'sunburst' => ['An interactive map of your disk', 'Every ring is a folder level. Hover to inspect, click to dive in, and watch the chart glide smoothly to the next level.'],
            'speed' => ['Fast. Really fast.', 'A parallel scanner written in Rust reads hundreds of thousands of files per second, counts the space files actually take on disk and never downloads files that only live in iCloud or other cloud folders.'],
            'hidden' => ['Hidden space, explained', 'Snapshots, swap and system files that no folder accounts for get their own segment, so the numbers finally add up.'],
            'largest' => ['Find the big stuff', 'Sorted folder lists, the largest files anywhere below the current folder, file type colors and instant search.'],
            'saved' => ['Scans are saved', 'Reopen your last scan instantly. A rescan only reads the folders that changed, and an interrupted scan continues where it stopped.'],
            'collector' => ['Clean up safely', 'Drag items into the Collector, review them and move them to the Trash in one go. Nothing is ever deleted permanently.'],
            'native' => ['At home on every platform', 'macOS on Apple Silicon and Intel, Windows on x64 and ARM, and every major Linux distribution. Light and dark mode, English and Dutch, and settings to make it your own.'],
            'private' => ['Private by design', 'Your files never leave your computer. No account, no telemetry; the only thing the app asks online is whether a new version exists.'],
        ],
    ],

    'how' => [
        'title' => 'Three steps to a tidy disk',
        'steps' => [
            ['Pick a drive or folder', 'Choose your system disk, an external drive or any folder. Scanning starts right away.'],
            ['Explore', 'Zoom through the sunburst, search, and sort by size or file type to find what you no longer need.'],
            ['Clean up', 'Collect files and folders, check the total and move them to the Trash. Restore them any time until you empty it.'],
        ],
    ],

    'showcase' => [
        'title' => 'Looks as good as it works',
        'lead' => 'Designed to feel at home in light and dark mode, with colors that tell you where your space went.',
        'dark' => 'Folder colors in dark mode',
        'light' => 'File type colors in light mode',
    ],

    'download' => [
        'title' => 'Download 2DriveScan',
        'lead' => 'Free for everyone. Pick your platform.',
        'page_lead' => 'Version :version, released on :date.',
        'page_lead_nodate' => 'Version :version.',
        'macos_req' => 'macOS 10.15 or later · Apple Silicon & Intel',
        'windows_req' => 'Windows 10 and 11 · x64 and ARM64',
        'linux_req' => 'Ubuntu, Debian, Fedora, openSUSE, Arch and more',
        'all' => 'All downloads and instructions',
        'installer' => 'Installer',
        'recommended' => 'Recommended',
        'checksum' => 'SHA-256',
        'unavailable' => 'Release information is temporarily unavailable. You can always get the installers from GitHub.',
        'on_github' => 'View this release on GitHub',
        'install_title' => 'Installing',
        'macos_steps' => [
            'Open the .dmg file and drag 2DriveScan to your Applications folder.',
            'The app is signed and notarized by Apple, so it opens without warnings.',
            'For a complete scan, allow Full Disk Access in System Settings → Privacy & Security.',
        ],
        'windows_steps' => [
            'Run the setup file. No administrator rights are needed.',
            'Windows SmartScreen may warn about a new app. Choose “More info” and then “Run anyway”.',
            'Prefer MSI packages? Use the .msi for x64 instead.',
        ],
        'linux_steps' => [
            'Debian, Ubuntu, Mint, Pop!_OS: install the .deb package.',
            'Fedora, openSUSE, RHEL: install the .rpm package.',
            'Any other distribution, including Arch: make the AppImage executable and run it.',
        ],
        'source_title' => 'Build from source',
        'source_text' => '2DriveScan is open source. Instructions for building it yourself are in the README on GitHub.',
    ],

    'faq' => [
        'title' => 'Questions & answers',
        'items' => [
            ['Is 2DriveScan really free?', 'Yes. 2DriveScan is open source under the MIT license. There is no paid version, no ads and no account.'],
            ['Is it an alternative to DaisyDisk?', 'Yes. 2DriveScan gives you the same kind of interactive disk map, and it runs on Windows and Linux too.'],
            ['Can it delete something important by accident?', 'No. 2DriveScan never deletes anything on its own. Items you collect are moved to the Trash or Recycle Bin only after you confirm, so you can always restore them.'],
            ['Why does macOS ask for Full Disk Access?', 'macOS protects some folders, such as Mail and Safari data. Without access, 2DriveScan counts them as hidden space. Allow Full Disk Access in System Settings → Privacy & Security for a complete picture.'],
            ['Do I have to wait for a full scan every time?', 'No. Every scan is saved, so you can reopen it straight away. A rescan only reads folders that changed since the last one, which usually takes seconds. If you stop a scan or quit the app, the next scan continues where it left off.'],
            ['What is “hidden space”?', 'Space that is in use on a drive but does not belong to a file you can see: local snapshots, swap, system files and folders you are not allowed to read.'],
            ['Does 2DriveScan send any data?', 'No. Scans happen entirely on your computer and nothing about your files is sent anywhere. The only online request is a check on GitHub for a new version when the app starts; you can switch that off in Settings.'],
            ['How do updates work?', 'From version 0.3.0, 2DriveScan tells you when a new version is available and installs it with one click, then restarts. You can also check yourself under Settings → Updates. Every update is signed, and the app refuses anything without a valid signature.'],
            ['Which Linux distributions are supported?', 'Any recent one. Use the .deb on Debian and Ubuntu based systems, the .rpm on Fedora and openSUSE, and the AppImage everywhere else, including Arch.'],
        ],
    ],

    'donate' => [
        'eyebrow' => 'Support 2DriveScan',
        'title' => 'Help cover the costs.',
        'text' => '2DriveScan is free and has no paid features. Making it available does cost money: the domain and the hosting of this website. A donation helps cover those costs. It is entirely voluntary and doesn\'t unlock anything.',
        'button' => 'Donate with PayPal',
        'note' => 'You donate on PayPal\'s own website. This site loads nothing from PayPal and sets no cookies.',
    ],

    'stats' => [
        'title' => 'Download statistics',
        'meta_description' => 'How often 2DriveScan has been downloaded: per platform, version and installer, from GitHub\'s download counters.',
        'lead' => 'How often 2DriveScan has been downloaded, per platform, version and installer.',
        'total_label' => 'installer downloads',
        'total_sub' => 'across :versions versions since :date',
        'by_os' => 'By platform',
        'share' => ':pct of all downloads',
        'by_version' => 'By version',
        'version' => 'Version',
        'released' => 'Released',
        'total' => 'Total',
        'by_variant' => 'By installer',
        'over_time' => 'Over time',
        'chart_label' => 'Total installer downloads from :from to :to',
        'history_since' => 'The history is recorded every hour since :date. The chart appears from the second day.',
        'history_none' => 'The history is recorded every hour. The chart appears from the second day.',
        'mac_updates' => 'macOS in-app updates: :count (not included above).',
        'about_title' => 'About these numbers',
        'about' => 'The numbers come from GitHub\'s own download counters for each installer. They include updates installed from within the app on Windows and Linux (those download the installer itself) and automated downloads, for example by bots and mirrors, so they are somewhat higher than the number of people. This website tracks no visitors and sets no cookies: the download buttons simply point to GitHub.',
        'unavailable' => 'The statistics are temporarily unavailable. GitHub shows the download counts on the releases page.',
        'downloaded' => 'Downloaded :count times',
        'link' => 'Statistics',
    ],

    'changelog' => [
        'title' => 'Changelog',
        'lead' => 'What changed in each version of 2DriveScan.',
        'empty' => 'No changes recorded yet.',
        'latest' => 'Latest',
    ],

    'footer' => [
        'tagline' => 'The free, open source disk space analyzer.',
        'made' => 'Made with Rust and Tauri. Released under the MIT license.',
        'product' => 'Product',
        'project' => 'Project',
        'source' => 'Source code',
        'releases' => 'All releases',
        'issues' => 'Report a problem',
        'license' => 'License',
        'stats' => 'Download statistics',
    ],
];
