<?php

namespace Tests\Feature;

use App\Services\Changelog;
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\Http;
use Tests\TestCase;

class SiteTest extends TestCase
{
    protected function setUp(): void
    {
        parent::setUp();
        Cache::flush();
    }

    private function fakeRelease(): void
    {
        $asset = fn (string $name, int $size) => [
            'name' => $name,
            'size' => $size,
            'browser_download_url' => "https://github.com/joostiphone/2drivescan/releases/download/v9.8.7/{$name}",
            'digest' => 'sha256:'.str_repeat('ab', 32),
        ];

        Http::fake([
            'api.github.com/*' => Http::response([
                'tag_name' => 'v9.8.7',
                'html_url' => 'https://github.com/joostiphone/2drivescan/releases/tag/v9.8.7',
                'published_at' => '2026-11-03T12:00:00Z',
                'assets' => [
                    $asset('2DriveScan_9.8.7_universal.dmg', 6_500_000),
                    $asset('2DriveScan_9.8.7_x64-setup.exe', 2_200_000),
                    $asset('2DriveScan_9.8.7_x64_en-US.msi', 3_300_000),
                    $asset('2DriveScan_9.8.7_arm64-setup.exe', 1_900_000),
                    $asset('2DriveScan_9.8.7_amd64.AppImage', 81_000_000),
                    $asset('2DriveScan_9.8.7_amd64.deb', 3_400_000),
                    $asset('2DriveScan-9.8.7-1.x86_64.rpm', 3_400_000),
                    $asset('2DriveScan_universal.app.tar.gz', 5_000_000),
                ],
            ]),
        ]);
    }

    public function test_home_page_renders_in_english_and_dutch(): void
    {
        $this->fakeRelease();

        $this->get('/')->assertOk()
            ->assertSee('See what fills your disk.')
            ->assertSee('Version 9.8.7')
            ->assertSee('hreflang="nl"', false);

        $this->get('/nl')->assertOk()
            ->assertSee('Zie wat je schijf vult.')
            ->assertSee('lang="nl"', false);
    }

    public function test_download_page_lists_installers(): void
    {
        $this->fakeRelease();

        $this->get('/download')->assertOk()
            ->assertSee('2DriveScan_9.8.7_universal.dmg')
            ->assertSee('2DriveScan_9.8.7_x64-setup.exe')
            ->assertSee('2DriveScan-9.8.7-1.x86_64.rpm')
            ->assertSee(str_repeat('ab', 32));

        $this->get('/nl/download')->assertOk()->assertSee('2DriveScan downloaden');
    }

    public function test_get_links_redirect_to_the_latest_installer(): void
    {
        $this->fakeRelease();

        $this->get('/get/mac')->assertRedirect('https://github.com/joostiphone/2drivescan/releases/download/v9.8.7/2DriveScan_9.8.7_universal.dmg');
        $this->get('/get/windows-arm64')->assertRedirectContains('_arm64-setup.exe');
        $this->get('/get/nonsense')->assertNotFound();
    }

    public function test_pages_still_work_when_github_is_down(): void
    {
        Http::fake(['api.github.com/*' => Http::response('unavailable', 503)]);

        $this->get('/')->assertOk()->assertSee('Get the latest version on GitHub');
        $this->get('/download')->assertOk()->assertSee('temporarily unavailable');
        $this->get('/get/mac')->assertRedirect('https://github.com/joostiphone/2drivescan/releases/latest');
    }

    public function test_last_good_release_is_used_during_an_outage(): void
    {
        $this->fakeRelease();
        $this->get('/')->assertSee('Version 9.8.7');

        // The regular cache expires, GitHub fails, the previous answer is kept.
        Cache::forget('github:joostiphone/2drivescan/releases/latest');
        Http::fake(['api.github.com/*' => Http::response('rate limited', 403)]);
        $this->get('/')->assertSee('Version 9.8.7');
    }

    public function test_changelog_and_sitemap(): void
    {
        $this->fakeRelease();

        $this->get('/changelog')->assertOk()->assertSee('v0.1.0');
        $this->get('/nl/changelog')->assertOk()->assertSee('De eerste publieke versie');
        $this->get('/sitemap.xml')->assertOk()
            ->assertHeader('Content-Type', 'application/xml')
            ->assertSee(url('/nl/download'), false);
    }

    public function test_changelog_parser_reads_versions_and_dates(): void
    {
        $entries = (new Changelog)->parse("# Changelog\n\nIntro\n\n## 1.2.0 – 2026-12-01\n\n- **New** thing\n\n## 1.1.0\n\n- Older\n");

        $this->assertCount(2, $entries);
        $this->assertSame('1.2.0', $entries[0]['version']);
        $this->assertSame('2026-12-01', $entries[0]['date']);
        $this->assertStringContainsString('<strong>New</strong>', $entries[0]['html']);
        $this->assertNull($entries[1]['date']);
    }
}
