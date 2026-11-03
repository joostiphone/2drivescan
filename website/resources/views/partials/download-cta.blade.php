@php
    // Primary download button. The server renders the macOS option; site.js
    // switches to the visitor's platform.
    $assets = $release['assets'] ?? [];
    $primary = ['macos' => 'mac', 'windows' => 'windows', 'linux' => 'appimage'];
    $options = [];
    foreach ($primary as $os => $key) {
        if (isset($assets[$key])) {
            $options[$os] = [
                'href' => route('get', $key),
                'label' => __('site.hero.download_for', ['os' => __('site.os.'.$os)]),
                'meta' => __('site.hero.version_line', ['version' => $release['version'], 'size' => human_bytes($assets[$key]['size'])]).' · '.$assets[$key]['format'],
            ];
        }
    }
    if ($options) {
        // Phones and tablets cannot run the app: send them to the download page.
        $options['mobile'] = [
            'href' => lroute('download'),
            'label' => __('site.nav.download'),
            'meta' => 'macOS · Windows · Linux',
        ];
    }
    $first = $options['macos'] ?? reset($options) ?: null;
@endphp
<div class="cta {{ $align ?? '' }}" data-download-cta data-options='@json($options)'>
    <div class="cta-buttons">
        @if ($first)
            <a class="btn btn-primary btn-lg" href="{{ $first['href'] }}" data-cta-link>
                @include('partials.icon', ['name' => 'download'])
                <span data-cta-label>{{ $first['label'] }}</span>
            </a>
        @else
            <a class="btn btn-primary btn-lg" href="{{ $releasesUrl }}" rel="noopener">
                @include('partials.icon', ['name' => 'github'])
                <span>{{ __('site.hero.unavailable') }}</span>
            </a>
        @endif
        <a class="btn btn-ghost btn-lg" href="{{ lroute('download') }}">{{ __('site.hero.other_platforms') }}</a>
    </div>
    @if ($first)
        <p class="cta-meta" data-cta-meta>{{ $first['meta'] }}</p>
    @endif
</div>
