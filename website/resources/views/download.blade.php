@extends('layouts.app')

@section('title', __('site.download.title').' — 2DriveScan')

@php
    $assets = $release['assets'] ?? [];
    $groups = [
        'macos' => ['icon' => 'command', 'req' => __('site.download.macos_req'), 'keys' => ['mac'], 'steps' => __('site.download.macos_steps')],
        'windows' => ['icon' => 'windows', 'req' => __('site.download.windows_req'), 'keys' => ['windows', 'windows-msi', 'windows-arm64'], 'steps' => __('site.download.windows_steps')],
        'linux' => ['icon' => 'terminal', 'req' => __('site.download.linux_req'), 'keys' => ['appimage', 'deb', 'rpm', 'appimage-arm64', 'deb-arm64', 'rpm-arm64'], 'steps' => __('site.download.linux_steps')],
    ];
    $date = $release && $release['published_at']
        ? $release['published_at']->locale(app()->getLocale())->isoFormat('D MMMM YYYY')
        : null;
@endphp

@section('content')
    <section class="page-hero">
        <div class="container">
            <h1>{{ __('site.download.title') }}</h1>
            @if ($release)
                <p class="lead">
                    {{ $date ? __('site.download.page_lead', ['version' => $release['version'], 'date' => $date]) : __('site.download.page_lead_nodate', ['version' => $release['version']]) }}
                    <a href="{{ $release['url'] }}" rel="noopener">{{ __('site.download.on_github') }} @include('partials.icon', ['name' => 'external'])</a>
                </p>
                @include('partials.download-cta', ['align' => 'center'])
                @if ($downloadTotal)
                    <p class="dl-stats">
                        {{ __('site.stats.downloaded', ['count' => nf($downloadTotal)]) }} ·
                        <a href="{{ lroute('stats') }}">{{ __('site.stats.link') }}</a>
                    </p>
                @endif
            @else
                <p class="lead">{{ __('site.download.unavailable') }}</p>
                <p><a class="btn btn-primary btn-lg" href="{{ $releasesUrl }}" rel="noopener">@include('partials.icon', ['name' => 'github']) GitHub</a></p>
            @endif
        </div>
    </section>

    <section class="section section-tight">
        <div class="container downloads">
            @foreach ($groups as $os => $g)
                <article class="dl-card reveal" id="{{ $os }}" data-os="{{ $os }}">
                    <header>
                        <div class="platform-icon">@include('partials.icon', ['name' => $g['icon']])</div>
                        <div>
                            <h2>{{ __('site.os.'.$os) }}</h2>
                            <p class="muted">{{ $g['req'] }}</p>
                        </div>
                    </header>

                    <div class="dl-body">
                    <div class="dl-files">
                        @foreach ($g['keys'] as $i => $key)
                            @isset($assets[$key])
                                @php $a = $assets[$key]; @endphp
                                <div class="dl-row">
                                    <div class="dl-name">
                                        <b>{{ $a['format'] }}</b> <span>{{ $a['arch'] }}</span>
                                        @if ($i === 0)<em class="badge">{{ __('site.download.recommended') }}</em>@endif
                                        <code>{{ $a['name'] }}</code>
                                        @if ($a['sha256'])
                                            <details class="sha"><summary>{{ __('site.download.checksum') }}</summary><code>{{ $a['sha256'] }}</code></details>
                                        @endif
                                    </div>
                                    <a @class(['btn', 'btn-primary' => $i === 0, 'btn-ghost' => $i > 0]) href="{{ route('get', $key) }}">
                                        @include('partials.icon', ['name' => 'download'])
                                        {{ human_bytes($a['size']) }}
                                    </a>
                                </div>
                            @endisset
                        @endforeach
                    </div>

                    <div>
                    <h3>{{ __('site.download.install_title') }}</h3>
                    <ol class="dl-steps">
                        @foreach ($g['steps'] as $step)
                            <li>{{ $step }}</li>
                        @endforeach
                    </ol>
                    @if ($os === 'linux' && $release)
                        <pre class="code"><code>sudo apt install ./{{ $assets['deb']['name'] ?? '2DriveScan_'.$release['version'].'_amd64.deb' }}
sudo dnf install ./{{ $assets['rpm']['name'] ?? '2DriveScan-'.$release['version'].'-1.x86_64.rpm' }}
chmod +x {{ $assets['appimage']['name'] ?? '2DriveScan_'.$release['version'].'_amd64.AppImage' }} && ./{{ $assets['appimage']['name'] ?? '2DriveScan_'.$release['version'].'_amd64.AppImage' }}</code></pre>
                    @endif
                    </div>
                    </div>
                </article>
            @endforeach

            <article class="dl-card dl-source reveal">
                <header>
                    <div class="platform-icon">@include('partials.icon', ['name' => 'code'])</div>
                    <div>
                        <h2>{{ __('site.download.source_title') }}</h2>
                        <p class="muted">{{ __('site.download.source_text') }}</p>
                    </div>
                </header>
                <a class="btn btn-ghost" href="{{ $repoUrl }}#development" rel="noopener">@include('partials.icon', ['name' => 'github']) GitHub</a>
            </article>
        </div>
    </section>
@endsection
