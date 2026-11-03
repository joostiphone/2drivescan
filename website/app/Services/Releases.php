<?php

namespace App\Services;

use Illuminate\Support\Carbon;
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Log;
use Throwable;

/**
 * Reads release information from GitHub so the website always offers the
 * newest installers without being redeployed.
 */
class Releases
{
    /**
     * Installer variants, matched against release asset names. The order is
     * the order they are shown in.
     */
    public const ASSETS = [
        'mac' => ['os' => 'macos', 'pattern' => '/_universal\.dmg$/', 'arch' => 'Apple Silicon & Intel', 'format' => '.dmg'],
        'windows' => ['os' => 'windows', 'pattern' => '/_x64-setup\.exe$/', 'arch' => 'x64', 'format' => '.exe'],
        'windows-msi' => ['os' => 'windows', 'pattern' => '/_x64_[^_]+\.msi$/', 'arch' => 'x64', 'format' => '.msi'],
        'windows-arm64' => ['os' => 'windows', 'pattern' => '/_arm64-setup\.exe$/', 'arch' => 'ARM64', 'format' => '.exe'],
        'appimage' => ['os' => 'linux', 'pattern' => '/_amd64\.AppImage$/', 'arch' => 'x64', 'format' => '.AppImage'],
        'deb' => ['os' => 'linux', 'pattern' => '/_amd64\.deb$/', 'arch' => 'x64', 'format' => '.deb'],
        'rpm' => ['os' => 'linux', 'pattern' => '/\.x86_64\.rpm$/', 'arch' => 'x64', 'format' => '.rpm'],
        'appimage-arm64' => ['os' => 'linux', 'pattern' => '/_aarch64\.AppImage$/', 'arch' => 'ARM64', 'format' => '.AppImage'],
        'deb-arm64' => ['os' => 'linux', 'pattern' => '/_arm64\.deb$/', 'arch' => 'ARM64', 'format' => '.deb'],
        'rpm-arm64' => ['os' => 'linux', 'pattern' => '/\.aarch64\.rpm$/', 'arch' => 'ARM64', 'format' => '.rpm'],
    ];

    public function repo(): string
    {
        return config('site.repo');
    }

    public function repoUrl(): string
    {
        return 'https://github.com/'.$this->repo();
    }

    public function releasesUrl(): string
    {
        return $this->repoUrl().'/releases/latest';
    }

    /**
     * The newest published release, normalized, or null when GitHub has never
     * been reachable.
     *
     * @return array{version: string, tag: string, url: string, published_at: ?Carbon, assets: array<string, array>}|null
     */
    public function latest(): ?array
    {
        $release = $this->fetch('/releases/latest');

        return $release ? $this->normalize($release) : null;
    }

    /** Download URL for an installer key, falling back to the releases page. */
    public function downloadUrl(string $key): string
    {
        return $this->latest()['assets'][$key]['url'] ?? $this->releasesUrl();
    }

    /** Forget cached data so the next request asks GitHub again. */
    public function refresh(): void
    {
        Cache::forget($this->cacheKey('/releases/latest'));
    }

    protected function normalize(array $release): array
    {
        $assets = [];
        foreach (self::ASSETS as $key => $meta) {
            foreach ($release['assets'] ?? [] as $asset) {
                if (preg_match($meta['pattern'], $asset['name'] ?? '')) {
                    $assets[$key] = $meta + [
                        'key' => $key,
                        'name' => $asset['name'],
                        'url' => $asset['browser_download_url'],
                        'size' => (int) ($asset['size'] ?? 0),
                        'sha256' => isset($asset['digest']) && str_starts_with($asset['digest'], 'sha256:')
                            ? substr($asset['digest'], 7)
                            : null,
                    ];
                    break;
                }
            }
        }

        $tag = $release['tag_name'] ?? '';

        return [
            'version' => ltrim($tag, 'v'),
            'tag' => $tag,
            'url' => $release['html_url'] ?? $this->releasesUrl(),
            'published_at' => isset($release['published_at']) ? Carbon::parse($release['published_at']) : null,
            'assets' => $assets,
        ];
    }

    protected function cacheKey(string $path): string
    {
        return 'github:'.$this->repo().$path;
    }

    /**
     * Cached GitHub API call. When GitHub is unreachable the last good answer
     * is used, so a short outage never breaks the download buttons.
     */
    protected function fetch(string $path): ?array
    {
        $key = $this->cacheKey($path);

        return Cache::remember($key, config('site.cache_ttl'), function () use ($key, $path) {
            try {
                $request = Http::acceptJson()
                    ->withHeaders(['User-Agent' => '2drivescan-website', 'X-GitHub-Api-Version' => '2022-11-28'])
                    ->timeout(6);
                if ($token = config('site.token')) {
                    $request = $request->withToken($token);
                }
                $response = $request->get('https://api.github.com/repos/'.$this->repo().$path);
                if ($response->successful()) {
                    Cache::forever($key.':last-good', $response->json());

                    return $response->json();
                }
                Log::warning('GitHub API returned '.$response->status().' for '.$path);
            } catch (Throwable $e) {
                Log::warning('GitHub API request failed: '.$e->getMessage());
            }

            return Cache::get($key.':last-good');
        });
    }
}
