@php
    $assets = $release['assets'] ?? [];
    $platforms = [
        'macos' => ['icon' => 'command', 'req' => __('site.download.macos_req'), 'main' => ['mac'], 'more' => []],
        'windows' => ['icon' => 'windows', 'req' => __('site.download.windows_req'), 'main' => ['windows'], 'more' => ['windows-msi', 'windows-arm64']],
        'linux' => ['icon' => 'terminal', 'req' => __('site.download.linux_req'), 'main' => ['appimage', 'deb', 'rpm'], 'more' => ['appimage-arm64', 'deb-arm64', 'rpm-arm64']],
    ];
@endphp
<div class="platforms">
    @foreach ($platforms as $os => $p)
        <article class="platform reveal" data-os="{{ $os }}">
            <div class="platform-icon">@include('partials.icon', ['name' => $p['icon']])</div>
            <h3>{{ __('site.os.'.$os) }}</h3>
            <p class="muted">{{ $p['req'] }}</p>
            <div class="platform-files">
                @php $shown = 0; @endphp
                @foreach ($p['main'] as $i => $key)
                    @isset($assets[$key])
                        @php $a = $assets[$key]; $shown++; @endphp
                        <a @class(['file', 'file-primary' => $i === 0]) href="{{ route('get', $key) }}">
                            @include('partials.icon', ['name' => 'download'])
                            <span class="file-label">{{ $a['format'] }} <small>{{ $a['arch'] }}</small></span>
                            <span class="file-size">{{ human_bytes($a['size']) }}</span>
                        </a>
                    @endisset
                @endforeach
                @if ($shown === 0)
                    <a class="file file-primary" href="{{ $releasesUrl }}" rel="noopener">
                        @include('partials.icon', ['name' => 'github'])
                        <span class="file-label">GitHub</span>
                    </a>
                @endif
            </div>
            @php $more = array_filter($p['more'], fn ($k) => isset($assets[$k])); @endphp
            @if ($more)
                <div class="platform-more">
                    @foreach ($more as $key)
                        <a href="{{ route('get', $key) }}">{{ $assets[$key]['format'] }} {{ $assets[$key]['arch'] }}</a>
                    @endforeach
                </div>
            @endif
        </article>
    @endforeach
</div>
