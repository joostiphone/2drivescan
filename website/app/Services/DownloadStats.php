<?php

namespace App\Services;

use Illuminate\Support\Carbon;
use Illuminate\Support\Facades\File;
use RuntimeException;

/**
 * Download statistics from GitHub's own download counters. The website does
 * not count anything itself: every download button redirects to GitHub.
 *
 * Only installers count (the patterns in Releases::ASSETS). Signatures,
 * latest.json and the macOS update bundle (.app.tar.gz, reported separately as
 * in-app updates) are left out.
 */
class DownloadStats
{
    public const OSES = ['macos', 'windows', 'linux'];

    /** The bundle macOS downloads for an in-app update. */
    public const MAC_UPDATE_PATTERN = '/\.app\.tar\.gz$/';

    /** Key for the macOS update bundle in the history file. */
    public const MAC_UPDATE_KEY = 'mac-update';

    public function __construct(private Releases $releases) {}

    /** Statistics from the cached release list, or null when GitHub has never been reachable. */
    public function summary(): ?array
    {
        $releases = $this->releases->all();

        return $releases === null ? null : $this->summarize($releases);
    }

    /**
     * Totals per platform, installer variant and version.
     *
     * @param  list<array>  $releases  releases as returned by the GitHub API
     * @return array{total: int, os: array<string, int>, variants: array<string, int>, versions: list<array>, mac_updates: int, first_release: ?Carbon}
     */
    public function summarize(array $releases): array
    {
        $os = array_fill_keys(self::OSES, 0);
        $variants = array_fill_keys(array_keys(Releases::ASSETS), 0);
        $versions = [];
        $macUpdates = 0;

        foreach ($releases as $release) {
            if (! is_array($release) || ! empty($release['draft'])) {
                continue;
            }
            $tag = (string) ($release['tag_name'] ?? '');
            $version = [
                'version' => ltrim($tag, 'v'),
                'tag' => $tag,
                'url' => $release['html_url'] ?? null,
                'published_at' => isset($release['published_at']) ? Carbon::parse($release['published_at']) : null,
                'total' => 0,
                'os' => array_fill_keys(self::OSES, 0),
                'assets' => [],
                'mac_updates' => 0,
            ];
            foreach ($release['assets'] ?? [] as $asset) {
                $name = (string) ($asset['name'] ?? '');
                $count = max(0, (int) ($asset['download_count'] ?? 0));
                if ($key = self::variantOf($name)) {
                    $assetOs = Releases::ASSETS[$key]['os'];
                    $version['assets'][$key] = ($version['assets'][$key] ?? 0) + $count;
                    $version['os'][$assetOs] += $count;
                    $version['total'] += $count;
                    $variants[$key] += $count;
                    $os[$assetOs] += $count;
                } elseif (preg_match(self::MAC_UPDATE_PATTERN, $name)) {
                    $version['mac_updates'] += $count;
                    $macUpdates += $count;
                }
            }
            $versions[] = $version;
        }

        // Newest first.
        usort($versions, fn (array $a, array $b) => ($b['published_at']?->getTimestamp() ?? 0) <=> ($a['published_at']?->getTimestamp() ?? 0));
        $dates = array_filter(array_column($versions, 'published_at'));

        return [
            'total' => array_sum($os),
            'os' => $os,
            'variants' => $variants,
            'versions' => $versions,
            'mac_updates' => $macUpdates,
            'first_release' => $dates ? min($dates) : null,
        ];
    }

    /** Installer key ("deb-arm64") for a release asset, or null for signatures and other files. */
    public static function variantOf(string $name): ?string
    {
        if (str_ends_with($name, '.sig')) {
            return null;
        }
        foreach (Releases::ASSETS as $key => $meta) {
            if (preg_match($meta['pattern'], $name)) {
                return $key;
            }
        }

        return null;
    }

    // ---------------------------------------------------------------- history

    public function historyPath(): string
    {
        return config('site.stats_history') ?: storage_path('app/downloads/history.json');
    }

    /**
     * Asks GitHub for the current counts and stores them as today's (UTC)
     * snapshot: cumulative counts per version per installer. Running it again
     * on the same day replaces that day. Returns the summary, or null when
     * GitHub could not be reached; then nothing is written.
     */
    public function snapshot(?Carbon $now = null): ?array
    {
        $releases = $this->releases->all(fresh: true);
        if ($releases === null) {
            return null;
        }
        $summary = $this->summarize($releases);
        $now = ($now ?? Carbon::now())->copy()->utc();

        $counts = [];
        foreach ($summary['versions'] as $version) {
            $row = $version['assets'];
            if ($version['mac_updates'] > 0) {
                $row[self::MAC_UPDATE_KEY] = $version['mac_updates'];
            }
            $counts[$version['version']] = $row;
        }

        $history = $this->readHistory();
        $history['days'][$now->toDateString()] = [
            'taken_at' => $now->format('Y-m-d\TH:i:s\Z'),
            'total' => $summary['total'],
            'counts' => $counts,
        ];
        ksort($history['days']);
        $this->writeHistory($history);

        return $summary;
    }

    /**
     * Cumulative installer downloads per day, oldest first.
     *
     * @return array<string, int> date (Y-m-d) => total
     */
    public function history(): array
    {
        $series = [];
        foreach ($this->readHistory()['days'] as $date => $day) {
            if (! is_array($day)) {
                continue;
            }
            $total = $day['total'] ?? null;
            if (! is_int($total)) {
                $total = 0;
                foreach ($day['counts'] ?? [] as $row) {
                    $total += array_sum(array_intersect_key((array) $row, Releases::ASSETS));
                }
            }
            $series[(string) $date] = $total;
        }
        ksort($series);

        return $series;
    }

    /** @return array{format: int, days: array<string, array>} */
    public function readHistory(): array
    {
        $empty = ['format' => 1, 'days' => []];
        $path = $this->historyPath();
        if (! is_file($path)) {
            return $empty;
        }
        $data = json_decode((string) file_get_contents($path), true);
        if (! is_array($data) || ! is_array($data['days'] ?? null)) {
            return $empty;
        }

        return ['format' => 1, 'days' => $data['days']];
    }

    /** Writes a temporary file next to the history and renames it, so readers never see half a file. */
    protected function writeHistory(array $history): void
    {
        $path = $this->historyPath();
        File::ensureDirectoryExists(dirname($path), 0775);
        $tmp = $path.'.'.getmypid().'.tmp';
        $json = json_encode($history, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR);
        if (file_put_contents($tmp, $json."\n") === false || ! rename($tmp, $path)) {
            @unlink($tmp);
            throw new RuntimeException("Could not write {$path}");
        }
        @chmod($path, 0664);
    }
}
