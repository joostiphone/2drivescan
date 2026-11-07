<?php

namespace Tests\Feature;

use App\Services\DownloadStats;
use Illuminate\Support\Carbon;
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\File;
use Illuminate\Support\Facades\Http;
use Tests\TestCase;

class DownloadStatsTest extends TestCase
{
    private string $dir;

    private int $extra = 0;

    private int $status = 200;

    protected function setUp(): void
    {
        parent::setUp();
        Cache::flush();
        $this->dir = sys_get_temp_dir().'/2ds-stats-'.uniqid();
        config(['site.stats_history' => $this->dir.'/downloads/history.json']);
    }

    protected function tearDown(): void
    {
        File::deleteDirectory($this->dir);
        parent::tearDown();
    }

    private function asset(string $name, int $count): array
    {
        return ['name' => $name, 'size' => 1000, 'download_count' => $count, 'browser_download_url' => "https://github.com/x/{$name}"];
    }

    /** Two published releases and a draft, with signatures and updater files mixed in. */
    private function releases(int $extra = 0): array
    {
        return [
            [
                'tag_name' => 'v0.2.0', 'draft' => false, 'published_at' => '2026-11-04T10:00:00Z', 'html_url' => 'https://github.com/x/v0.2.0',
                'assets' => [
                    $this->asset('2DriveScan_0.2.0_universal.dmg', 10 + $extra),
                    $this->asset('2DriveScan_0.2.0_universal.dmg.sig', 99),
                    $this->asset('2DriveScan_universal.app.tar.gz', 4),
                    $this->asset('2DriveScan_universal.app.tar.gz.sig', 99),
                    $this->asset('2DriveScan_0.2.0_x64-setup.exe', 7),
                    $this->asset('2DriveScan_0.2.0_x64-setup.exe.sig', 99),
                    $this->asset('2DriveScan_0.2.0_x64_en-US.msi', 3),
                    $this->asset('2DriveScan_0.2.0_arm64-setup.exe', 2),
                    $this->asset('2DriveScan_0.2.0_amd64.AppImage', 5),
                    $this->asset('2DriveScan_0.2.0_arm64.deb', 1),
                    $this->asset('2DriveScan-0.2.0-1.x86_64.rpm', 6),
                    $this->asset('latest.json', 500),
                    $this->asset('notes.txt', 50),
                ],
            ],
            [
                'tag_name' => 'v0.3.0-draft', 'draft' => true, 'published_at' => null,
                'assets' => [$this->asset('2DriveScan_0.3.0_universal.dmg', 1000)],
            ],
            [
                'tag_name' => 'v0.1.0', 'draft' => false, 'published_at' => '2026-11-01T10:00:00Z',
                'assets' => [
                    $this->asset('2DriveScan_0.1.0_universal.dmg', 2),
                    $this->asset('2DriveScan_0.1.0_aarch64.AppImage', 1),
                    $this->asset('2DriveScan-0.1.0-1.aarch64.rpm', 1),
                ],
            ],
        ];
    }

    /** Fakes GitHub; later calls only change the numbers or the status the fake answers with. */
    private function fakeGitHub(int $extra = 0, int $status = 200): void
    {
        [$this->extra, $this->status] = [$extra, $status];
        Http::fake(fn ($request) => match (true) {
            $this->status !== 200 => Http::response('down', $this->status),
            str_contains($request->url(), '/releases/latest') => Http::response($this->releases($this->extra)[0]),
            default => Http::response($this->releases($this->extra)),
        });
    }

    public function test_only_installers_of_published_releases_are_counted(): void
    {
        $s = app(DownloadStats::class)->summarize($this->releases());

        $this->assertSame(38, $s['total']);
        $this->assertSame(['macos' => 12, 'windows' => 12, 'linux' => 14], $s['os']);
        $this->assertSame(12, $s['variants']['mac']);
        $this->assertSame(7, $s['variants']['windows']);
        $this->assertSame(3, $s['variants']['windows-msi']);
        $this->assertSame(1, $s['variants']['rpm-arm64']);
        $this->assertSame(0, $s['variants']['deb']);
        $this->assertSame(4, $s['mac_updates']);
        $this->assertSame(['0.2.0', '0.1.0'], array_column($s['versions'], 'version'));
        $this->assertSame(34, $s['versions'][0]['total']);
        $this->assertSame('2026-11-01', $s['first_release']->toDateString());
    }

    public function test_asset_names_map_to_installer_variants(): void
    {
        $this->assertSame('mac', DownloadStats::variantOf('2DriveScan_1.0.0_universal.dmg'));
        $this->assertSame('deb-arm64', DownloadStats::variantOf('2DriveScan_1.0.0_arm64.deb'));
        $this->assertSame('rpm', DownloadStats::variantOf('2DriveScan-1.0.0-1.x86_64.rpm'));
        $this->assertNull(DownloadStats::variantOf('2DriveScan_1.0.0_universal.dmg.sig'));
        $this->assertNull(DownloadStats::variantOf('2DriveScan_universal.app.tar.gz'));
        $this->assertNull(DownloadStats::variantOf('latest.json'));
    }

    public function test_drafts_from_the_api_are_skipped(): void
    {
        $this->fakeGitHub();

        $this->assertSame(38, app(DownloadStats::class)->summary()['total']);
    }

    public function test_snapshot_writes_one_entry_per_day_and_replaces_the_same_day(): void
    {
        $stats = app(DownloadStats::class);
        $path = $stats->historyPath();

        $this->fakeGitHub();
        $stats->snapshot(Carbon::parse('2026-11-07 09:15:00', 'UTC'));
        $this->fakeGitHub(extra: 5);
        $stats->snapshot(Carbon::parse('2026-11-07 10:15:00', 'UTC'));

        $data = json_decode(File::get($path), true);
        $this->assertSame(['2026-11-07'], array_keys($data['days']));
        $this->assertSame(43, $data['days']['2026-11-07']['total']);
        $this->assertSame('2026-11-07T10:15:00Z', $data['days']['2026-11-07']['taken_at']);
        $this->assertSame(15, $data['days']['2026-11-07']['counts']['0.2.0']['mac']);
        $this->assertSame(4, $data['days']['2026-11-07']['counts']['0.2.0']['mac-update']);
        $this->assertArrayNotHasKey('0.3.0-draft', $data['days']['2026-11-07']['counts']);

        $stats->snapshot(Carbon::parse('2026-11-08 00:30:00', 'UTC'));
        $this->assertSame(['2026-11-07' => 43, '2026-11-08' => 43], $stats->history());

        // Written through a temporary file that is renamed, so none is left behind.
        $this->assertSame(['history.json'], array_map('basename', File::files(dirname($path))));
    }

    public function test_snapshot_writes_nothing_when_github_is_down(): void
    {
        $this->fakeGitHub(status: 503);

        $this->artisan('downloads:snapshot')
            ->expectsOutputToContain('GitHub could not be reached')
            ->assertFailed();
        $this->assertFileDoesNotExist(app(DownloadStats::class)->historyPath());
    }

    public function test_snapshot_command_prints_the_totals(): void
    {
        $this->fakeGitHub();

        $this->artisan('downloads:snapshot')
            ->expectsOutputToContain('Installer downloads: 38 (macos 12, windows 12, linux 14); macOS in-app updates: 4.')
            ->assertSuccessful();
        $this->assertFileExists(app(DownloadStats::class)->historyPath());
    }

    public function test_stats_page_in_english_and_dutch(): void
    {
        $this->fakeGitHub();

        $this->get('/stats')->assertOk()
            ->assertSee('Download statistics')
            ->assertSee('installer downloads')
            ->assertSee('v0.2.0')
            ->assertDontSee('0.3.0-draft')
            ->assertSee('The history is recorded every hour.');
        $this->get('/nl/stats')->assertOk()
            ->assertSee('Downloadstatistieken')
            ->assertSee('lang="nl"', false);
    }

    public function test_stats_page_shows_the_number_with_the_thousands_separator_of_the_language(): void
    {
        $this->fakeGitHub(extra: 1224);

        $this->get('/stats')->assertSee('1,262');
        $this->get('/nl/stats')->assertSee('1.262');
    }

    public function test_chart_appears_from_the_second_day(): void
    {
        $this->fakeGitHub();
        $stats = app(DownloadStats::class);

        $stats->snapshot(Carbon::parse('2026-11-07 12:00:00', 'UTC'));
        $this->get('/stats')->assertDontSee('class="chart"', false)->assertSee('since 7 November 2026');

        $stats->snapshot(Carbon::parse('2026-11-08 12:00:00', 'UTC'));
        $this->get('/stats')->assertSee('<svg class="chart"', false)->assertSee('8 Nov');
    }

    public function test_links_to_the_statistics_and_no_cookies(): void
    {
        $this->fakeGitHub();

        $download = $this->get('/download')->assertOk()
            ->assertSee('Downloaded 38 times')
            ->assertSee('href="'.url('/stats').'"', false);
        $this->assertSame([], $download->headers->getCookies());

        $stats = $this->get('/stats')->assertOk();
        $this->assertSame([], $stats->headers->getCookies());
        // Scripts and styles only come from the site itself.
        preg_match_all('#<(?:script|link)[^>]+(?:src|href)="(https?://[^"]+)"#', $stats->getContent(), $m);
        foreach ($m[1] as $url) {
            $this->assertStringStartsWith(url('/'), $url);
        }

        $this->get('/')->assertSee('href="'.url('/stats').'"', false);
        $this->get('/sitemap.xml')->assertSee(url('/stats'), false)->assertSee(url('/nl/stats'), false);
    }

    public function test_structured_data_is_valid_json(): void
    {
        $this->fakeGitHub();
        $html = $this->get('/')->assertOk()->getContent();

        $this->assertSame(1, preg_match('#<script type="application/ld\+json">(.*?)</script>#s', $html, $m));
        $data = json_decode(trim($m[1]), true);
        $this->assertSame('https://schema.org', $data['@context'] ?? null);
        $this->assertSame('SoftwareApplication', $data['@type']);
        $this->assertSame('0.2.0', $data['softwareVersion']);
    }
}
