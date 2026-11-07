@extends('layouts.app')

@section('title', __('site.stats.title').' — 2DriveScan')
@section('description', __('site.stats.meta_description'))

@php
    $locale = app()->getLocale();
    $icons = ['macos' => 'command', 'windows' => 'windows', 'linux' => 'terminal'];
    $pct = function (int $part, int $whole) use ($locale): string {
        if ($whole <= 0) {
            return '0%';
        }
        $p = $part / $whole * 100;

        return number_format($p, $p > 0 && $p < 10 ? 1 : 0, $locale === 'nl' ? ',' : '.', '').'%';
    };
    $date = fn ($d, string $format = 'D MMMM YYYY') => $d ? $d->copy()->locale($locale)->isoFormat($format) : '';
@endphp

@section('content')
    <section class="page-hero">
        <div class="container">
            <h1>{{ __('site.stats.title') }}</h1>
            <p class="lead">{{ __('site.stats.lead') }}</p>
        </div>
    </section>

    @if (! $stats)
        <section class="section section-tight">
            <div class="container narrow center">
                <p class="lead">{{ __('site.stats.unavailable') }}</p>
                <p><a class="btn btn-primary" href="{{ $repoUrl }}/releases" rel="noopener">@include('partials.icon', ['name' => 'github']) GitHub</a></p>
            </div>
        </section>
    @else
        @php
            $total = $stats['total'];
            $maxVariant = max(1, max($stats['variants']));
        @endphp

        <section class="section section-tight">
            <div class="container">
                <div class="stat-total reveal">
                    <div class="stat-number">{{ nf($total) }}</div>
                    <div class="stat-label">{{ __('site.stats.total_label') }}</div>
                    @if ($stats['first_release'])
                        <div class="stat-sub">{{ __('site.stats.total_sub', ['versions' => count($stats['versions']), 'date' => $date($stats['first_release'])]) }}</div>
                    @endif
                </div>

                {{-- Per platform --}}
                <h2 class="stat-h">{{ __('site.stats.by_os') }}</h2>
                <div class="os-grid">
                    @foreach ($stats['os'] as $os => $n)
                        <div class="os-card os-{{ $os }} reveal">
                            <div class="platform-icon">@include('partials.icon', ['name' => $icons[$os]])</div>
                            <div class="os-info">
                                <h3>{{ __('site.os.'.$os) }}</h3>
                                <div class="os-count">{{ nf($n) }}</div>
                                <div class="share" role="img" aria-label="{{ __('site.stats.share', ['pct' => $pct($n, $total)]) }}"><span style="width: {{ $total ? round($n / $total * 100, 1) : 0 }}%"></span></div>
                                <div class="muted small">{{ __('site.stats.share', ['pct' => $pct($n, $total)]) }}</div>
                            </div>
                        </div>
                    @endforeach
                </div>

                {{-- Over time --}}
                <h2 class="stat-h">{{ __('site.stats.over_time') }}</h2>
                <div class="chart-card reveal">
                    @if ($chart)
                        @php
                            $from = $date(\Illuminate\Support\Carbon::parse(array_key_first($series)));
                            $to = $date(\Illuminate\Support\Carbon::parse(array_key_last($series)));
                        @endphp
                        <svg class="chart" viewBox="0 0 {{ $chart['width'] }} {{ $chart['height'] }}" role="img" aria-labelledby="chart-title">
                            <title id="chart-title">{{ __('site.stats.chart_label', ['from' => $from, 'to' => $to]) }}</title>
                            <defs>
                                <linearGradient id="chart-fill" x1="0" y1="0" x2="0" y2="1">
                                    <stop offset="0" stop-color="#b64dff" stop-opacity="0.35"/>
                                    <stop offset="1" stop-color="#7c5cff" stop-opacity="0"/>
                                </linearGradient>
                                <linearGradient id="chart-line" x1="0" y1="0" x2="1" y2="0">
                                    <stop offset="0" stop-color="#7c5cff"/>
                                    <stop offset="0.5" stop-color="#b64dff"/>
                                    <stop offset="1" stop-color="#ff4fa3"/>
                                </linearGradient>
                            </defs>
                            @foreach ($chart['yticks'] as $tick)
                                <line class="grid" x1="{{ $chart['left'] }}" x2="{{ $chart['right'] }}" y1="{{ $tick['y'] }}" y2="{{ $tick['y'] }}" vector-effect="non-scaling-stroke"/>
                                <text class="axis" x="{{ $chart['left'] - 10 }}" y="{{ $tick['y'] }}" text-anchor="end" dominant-baseline="middle">{{ nf($tick['value']) }}</text>
                            @endforeach
                            @foreach ($chart['xticks'] as $i => $tick)
                                <text class="axis" x="{{ $tick['x'] }}" y="{{ $chart['bottom'] + 12 }}" dominant-baseline="hanging" text-anchor="{{ $i === 0 ? 'start' : ($loop->last ? 'end' : 'middle') }}">{{ $tick['label'] }}</text>
                            @endforeach
                            <path d="{{ $chart['area'] }}" fill="url(#chart-fill)"/>
                            <path d="{{ $chart['line'] }}" fill="none" stroke="url(#chart-line)" stroke-width="2.5" stroke-linejoin="round" stroke-linecap="round" vector-effect="non-scaling-stroke"/>
                            @foreach ($chart['points'] as $p)
                                <circle class="dot" cx="{{ $p['x'] }}" cy="{{ $p['y'] }}" r="{{ $loop->last ? 4.5 : 3 }}"><title>{{ $date(\Illuminate\Support\Carbon::parse($p['date']), 'D MMM YYYY') }}: {{ nf($p['value']) }}</title></circle>
                            @endforeach
                        </svg>
                    @elseif ($series)
                        <p class="muted">{{ __('site.stats.history_since', ['date' => $date(\Illuminate\Support\Carbon::parse(array_key_first($series)))]) }}</p>
                    @else
                        <p class="muted">{{ __('site.stats.history_none') }}</p>
                    @endif
                </div>

                {{-- Per version --}}
                <h2 class="stat-h">{{ __('site.stats.by_version') }}</h2>
                <div class="table-wrap reveal">
                    <table class="stat-table">
                        <thead>
                            <tr>
                                <th scope="col">{{ __('site.stats.version') }}</th>
                                <th scope="col" class="col-date">{{ __('site.stats.released') }}</th>
                                @foreach (\App\Services\DownloadStats::OSES as $os)
                                    <th scope="col" class="num col-os">{{ __('site.os.'.$os) }}</th>
                                @endforeach
                                <th scope="col" class="num">{{ __('site.stats.total') }}</th>
                            </tr>
                        </thead>
                        <tbody>
                            @foreach ($stats['versions'] as $v)
                                <tr>
                                    <th scope="row">
                                        <a href="{{ lroute('changelog') }}#v{{ $v['version'] }}">v{{ $v['version'] }}</a>
                                        @if ($loop->first)<em class="badge">{{ __('site.changelog.latest') }}</em>@endif
                                    </th>
                                    <td class="col-date">@if ($v['published_at'])<time datetime="{{ $v['published_at']->toDateString() }}">{{ $date($v['published_at'], 'D MMM YYYY') }}</time>@endif</td>
                                    @foreach ($v['os'] as $os => $n)
                                        <td class="num col-os">{{ nf($n) }}</td>
                                    @endforeach
                                    <td class="num">
                                        <b>{{ nf($v['total']) }}</b>
                                        <span class="stack" aria-hidden="true">
                                            @foreach ($v['os'] as $os => $n)
                                                <span class="os-{{ $os }}" style="width: {{ $v['total'] ? round($n / $v['total'] * 100, 1) : 0 }}%"></span>
                                            @endforeach
                                        </span>
                                    </td>
                                </tr>
                            @endforeach
                        </tbody>
                    </table>
                </div>

                {{-- Per installer --}}
                <h2 class="stat-h">{{ __('site.stats.by_variant') }}</h2>
                <div class="variant-grid">
                    @foreach (\App\Services\DownloadStats::OSES as $os)
                        <div class="variant-card os-{{ $os }} reveal">
                            <h3>@include('partials.icon', ['name' => $icons[$os]]) {{ __('site.os.'.$os) }}</h3>
                            @foreach (\App\Services\Releases::ASSETS as $key => $meta)
                                @continue($meta['os'] !== $os)
                                <div class="variant-row">
                                    <span class="variant-name"><b>{{ $meta['format'] }}</b> {{ $meta['arch'] }}</span>
                                    <span class="variant-count">{{ nf($stats['variants'][$key]) }}</span>
                                    <span class="bar" aria-hidden="true"><span style="width: {{ round($stats['variants'][$key] / $maxVariant * 100, 1) }}%"></span></span>
                                </div>
                            @endforeach
                        </div>
                    @endforeach
                </div>

                <div class="stat-about reveal">
                    <h2>{{ __('site.stats.about_title') }}</h2>
                    <p>{{ __('site.stats.about') }}</p>
                    @if ($stats['mac_updates'] > 0)
                        <p class="muted">{{ __('site.stats.mac_updates', ['count' => nf($stats['mac_updates'])]) }}</p>
                    @endif
                </div>
            </div>
        </section>
    @endif
@endsection
