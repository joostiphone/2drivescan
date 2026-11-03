@extends('layouts.app')

@php
    $featureIcons = [
        'sunburst' => 'pie', 'speed' => 'bolt', 'hidden' => 'eye', 'largest' => 'search',
        'types' => 'layers', 'collector' => 'trash', 'native' => 'monitor', 'private' => 'shield',
    ];
    $demoSrc = asset('demo/index.html').'?demo=scan&path=%2F&label=Macintosh%20HD&theme=dark';
@endphp

@section('content')
    {{-- Hero ------------------------------------------------------------ --}}
    <section class="hero">
        <div class="container hero-text">
            <p class="eyebrow">{{ __('site.hero.eyebrow') }}</p>
            <h1>{{ __('site.hero.title') }} <span class="grad">{{ __('site.hero.title_highlight') }}</span></h1>
            <p class="lead">{{ __('site.hero.lead') }}</p>
            @include('partials.download-cta', ['align' => 'center'])
        </div>

        <div class="container" id="demo">
            <figure class="window" data-demo>
                <div class="window-bar">
                    <span class="dots" aria-hidden="true"><i></i><i></i><i></i></span>
                    <span class="window-title">2DriveScan</span>
                    <a class="window-action" href="{{ $demoSrc }}" target="_blank" rel="noopener" title="{{ __('site.hero.open_fullscreen') }}">
                        @include('partials.icon', ['name' => 'expand'])
                    </a>
                </div>
                <div class="window-body" data-demo-frame>
                    <img class="demo-poster" src="{{ asset('img/screens/result-dark.png') }}" alt="{{ __('site.showcase.dark') }}" width="2640" height="1720">
                    <iframe title="2DriveScan live demo" data-src="{{ $demoSrc }}" loading="lazy" tabindex="-1"></iframe>
                    <button class="demo-start" type="button" data-demo-start>
                        @include('partials.icon', ['name' => 'play'])
                        {{ __('site.hero.try_demo') }}
                    </button>
                </div>
            </figure>
            <figcaption class="demo-caption">
                {{ __('site.hero.demo_caption') }}
                <a href="{{ $demoSrc }}" target="_blank" rel="noopener">{{ __('site.hero.open_fullscreen') }} @include('partials.icon', ['name' => 'arrow'])</a>
            </figcaption>
        </div>
    </section>

    {{-- Strip ------------------------------------------------------------ --}}
    <section class="strip">
        <div class="container strip-inner">
            <span>@include('partials.icon', ['name' => 'rust']) {{ __('site.strip.rust') }}</span>
            <span>@include('partials.icon', ['name' => 'feather']) {{ __('site.strip.tiny') }}</span>
            <span>@include('partials.icon', ['name' => 'shield']) {{ __('site.strip.private') }}</span>
            <span>@include('partials.icon', ['name' => 'wifi-off']) {{ __('site.strip.offline') }}</span>
            <span>@include('partials.icon', ['name' => 'code']) {{ __('site.strip.open') }}</span>
        </div>
    </section>

    {{-- Features --------------------------------------------------------- --}}
    <section class="section" id="features">
        <div class="container">
            <header class="section-head">
                <h2>{{ __('site.features.title') }}</h2>
                <p>{{ __('site.features.lead') }}</p>
            </header>
            <div class="features">
                @foreach (__('site.features.items') as $key => [$title, $text])
                    <article class="feature reveal">
                        <div class="feature-icon feature-{{ $key }}">@include('partials.icon', ['name' => $featureIcons[$key]])</div>
                        <h3>{{ $title }}</h3>
                        <p>{{ $text }}</p>
                    </article>
                @endforeach
            </div>
        </div>
    </section>

    {{-- How it works ----------------------------------------------------- --}}
    <section class="section section-tight">
        <div class="container">
            <header class="section-head">
                <h2>{{ __('site.how.title') }}</h2>
            </header>
            <ol class="steps">
                @foreach (__('site.how.steps') as $i => [$title, $text])
                    <li class="step reveal">
                        <span class="step-num">{{ $i + 1 }}</span>
                        <h3>{{ $title }}</h3>
                        <p>{{ $text }}</p>
                    </li>
                @endforeach
            </ol>
        </div>
    </section>

    {{-- Showcase --------------------------------------------------------- --}}
    <section class="section">
        <div class="container">
            <header class="section-head">
                <h2>{{ __('site.showcase.title') }}</h2>
                <p>{{ __('site.showcase.lead') }}</p>
            </header>
            <div class="showcase">
                <figure class="shot reveal">
                    <img src="{{ asset('img/screens/result-dark.png') }}" alt="{{ __('site.showcase.dark') }}" loading="lazy" width="2640" height="1720">
                    <figcaption>{{ __('site.showcase.dark') }}</figcaption>
                </figure>
                <figure class="shot reveal">
                    <img src="{{ asset('img/screens/types-light.png') }}" alt="{{ __('site.showcase.light') }}" loading="lazy" width="2640" height="1720">
                    <figcaption>{{ __('site.showcase.light') }}</figcaption>
                </figure>
            </div>
        </div>
    </section>

    {{-- Download --------------------------------------------------------- --}}
    <section class="section" id="download">
        <div class="container">
            <header class="section-head">
                <h2>{{ __('site.download.title') }}</h2>
                <p>{{ __('site.download.lead') }}</p>
            </header>
            @include('partials.platforms')
            <p class="center"><a class="link-arrow" href="{{ lroute('download') }}">{{ __('site.download.all') }} @include('partials.icon', ['name' => 'arrow'])</a></p>
        </div>
    </section>

    {{-- FAQ -------------------------------------------------------------- --}}
    <section class="section section-tight" id="faq">
        <div class="container narrow">
            <header class="section-head">
                <h2>{{ __('site.faq.title') }}</h2>
            </header>
            <div class="faq">
                @foreach (__('site.faq.items') as [$q, $a])
                    <details class="reveal">
                        <summary>{{ $q }} @include('partials.icon', ['name' => 'chevron'])</summary>
                        <p>{{ $a }}</p>
                    </details>
                @endforeach
            </div>
        </div>
    </section>

    {{-- Closing CTA ------------------------------------------------------ --}}
    <section class="section closing">
        <div class="container closing-inner">
            <img src="{{ asset('img/logo.png') }}" alt="" width="88" height="88">
            <h2>{{ __('site.hero.title_highlight') }}</h2>
            @include('partials.download-cta', ['align' => 'center'])
        </div>
    </section>
@endsection
