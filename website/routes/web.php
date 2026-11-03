<?php

use App\Http\Controllers\SiteController;
use Illuminate\Support\Facades\Route;

// Every page exists once per language: /download (English) and /nl/download.
$pages = function (string $locale): void {
    Route::get('/', [SiteController::class, 'home'])->name("$locale.home");
    Route::get('/download', [SiteController::class, 'download'])->name("$locale.download");
    Route::get('/changelog', [SiteController::class, 'changelog'])->name("$locale.changelog");
};

foreach (array_keys(config('site.locales')) as $i => $locale) {
    Route::middleware("locale:$locale")
        ->prefix($i === 0 ? '' : $locale)
        ->group(fn () => $pages($locale));
}

Route::get('/get/{platform}', [SiteController::class, 'file'])->name('get');
Route::get('/sitemap.xml', [SiteController::class, 'sitemap'])->name('sitemap');
