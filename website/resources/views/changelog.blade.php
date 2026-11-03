@extends('layouts.app')

@section('title', __('site.changelog.title').' — 2DriveScan')

@section('content')
    <section class="page-hero">
        <div class="container">
            <h1>{{ __('site.changelog.title') }}</h1>
            <p class="lead">{{ __('site.changelog.lead') }}</p>
        </div>
    </section>

    <section class="section section-tight">
        <div class="container narrow">
            @forelse ($entries as $i => $entry)
                <article class="release reveal" id="v{{ $entry['version'] }}">
                    <header>
                        <h2>v{{ $entry['version'] }}</h2>
                        @if ($i === 0)<em class="badge">{{ __('site.changelog.latest') }}</em>@endif
                        @if ($entry['date'])
                            <time datetime="{{ $entry['date'] }}">{{ \Illuminate\Support\Carbon::parse($entry['date'])->locale(app()->getLocale())->isoFormat('D MMMM YYYY') }}</time>
                        @endif
                    </header>
                    <div class="prose">{!! $entry['html'] !!}</div>
                </article>
            @empty
                <p class="empty">{{ __('site.changelog.empty') }}</p>
            @endforelse
        </div>
    </section>
@endsection
