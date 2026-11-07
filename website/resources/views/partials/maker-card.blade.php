{{-- 2Secret, the maker's other project: a plain link, the icon is served from this site. --}}
<div class="maker-card reveal">
    <img src="{{ asset('img/2secret.png') }}" alt="" width="64" height="64" loading="lazy">
    <div class="maker-text">
        <h3>{{ __('site.more.name') }}</h3>
        <p class="maker-pitch">{{ __('site.more.pitch') }}</p>
        <p class="maker-desc">{{ __('site.more.text') }}</p>
        <p class="maker-platforms">@include('partials.icon', ['name' => 'monitor']) {{ __('site.more.platforms') }}</p>
    </div>
    <a class="btn btn-ghost" href="{{ __('site.more.url') }}" rel="noopener">
        {{ __('site.more.button') }}
        @include('partials.icon', ['name' => 'external'])
    </a>
</div>
