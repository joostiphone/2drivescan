<?php

namespace App\Http\Controllers;

use App\Services\Changelog;
use App\Services\DownloadStats;
use App\Services\Releases;
use App\Support\LineChart;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Response;
use Illuminate\View\View;

class SiteController extends Controller
{
    public function __construct(private Releases $releases) {}

    public function home(): View
    {
        return view('home', $this->shared());
    }

    public function download(DownloadStats $stats): View
    {
        return view('download', $this->shared() + [
            'downloadTotal' => $stats->summary()['total'] ?? null,
        ]);
    }

    /** Public download statistics, from GitHub's download counters. */
    public function stats(DownloadStats $stats): View
    {
        $series = $stats->history();

        return view('stats', $this->shared() + [
            'stats' => $stats->summary(),
            'series' => $series,
            'chart' => LineChart::build($series),
        ]);
    }

    public function changelog(Changelog $changelog): View
    {
        return view('changelog', $this->shared() + [
            'entries' => $changelog->entries(app()->getLocale()),
        ]);
    }

    /** Stable links like /get/mac that always point at the newest installer. */
    public function file(string $platform): RedirectResponse
    {
        abort_unless(array_key_exists($platform, Releases::ASSETS), 404);

        return redirect()->away($this->releases->downloadUrl($platform));
    }

    public function sitemap(): Response
    {
        $urls = [];
        foreach (array_keys(config('site.locales')) as $locale) {
            foreach (['home', 'download', 'changelog', 'stats'] as $page) {
                $urls[] = lroute($page, $locale);
            }
        }

        return response()
            ->view('sitemap', ['urls' => $urls])
            ->header('Content-Type', 'application/xml');
    }

    private function shared(): array
    {
        return [
            'release' => $this->releases->latest(),
            'repoUrl' => $this->releases->repoUrl(),
            'releasesUrl' => $this->releases->releasesUrl(),
        ];
    }
}
