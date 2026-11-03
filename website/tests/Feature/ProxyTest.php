<?php

namespace Tests\Feature;

use Illuminate\Http\Middleware\TrustProxies;
use Illuminate\Support\Facades\Http;
use Tests\TestCase;

class ProxyTest extends TestCase
{
    protected function setUp(): void
    {
        parent::setUp();
        Http::fake(['api.github.com/*' => Http::response('unavailable', 503)]);
    }

    public function test_links_use_https_behind_the_local_tunnel(): void
    {
        $this->withServerVariables(['REMOTE_ADDR' => '127.0.0.1'])
            ->withHeaders(['X-Forwarded-Proto' => 'https'])
            ->get('http://2drivescan.app/')
            ->assertOk()
            ->assertSee('https://2drivescan.app/download', false)
            ->assertDontSee('http://2drivescan.app/download', false);
    }

    public function test_www_redirects_to_the_bare_domain_in_production(): void
    {
        config(['app.url' => 'https://2drivescan.app']);
        $this->app->detectEnvironment(fn () => 'production');

        $this->get('http://www.2drivescan.app/nl/download?x=1')
            ->assertStatus(301)
            ->assertRedirect('https://2drivescan.app/nl/download?x=1');
    }

    public function test_a_tunnel_connector_elsewhere_in_the_network_can_be_trusted(): void
    {
        config(['site.trusted_proxies' => ['127.0.0.1', '192.168.1.93']]);
        TrustProxies::at(config('site.trusted_proxies'));

        $this->withServerVariables(['REMOTE_ADDR' => '192.168.1.93'])
            ->withHeaders(['X-Forwarded-Proto' => 'https', 'X-Forwarded-For' => '203.0.113.7'])
            ->get('http://2drivescan.app/up')
            ->assertOk();

        $this->assertTrue(request()->isSecure());
        $this->assertSame('203.0.113.7', request()->ip());
    }

    public function test_pages_set_no_cookies(): void
    {
        $response = $this->get('/');

        $response->assertOk();
        $this->assertSame([], $response->headers->getCookies());
    }
}
