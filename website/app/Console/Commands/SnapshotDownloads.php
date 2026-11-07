<?php

namespace App\Console\Commands;

use App\Services\DownloadStats;
use Illuminate\Console\Command;

/**
 * Records today's download counts for the history chart on /stats. Safe to
 * run as often as you like: each run replaces today's (UTC) snapshot. On the
 * server an hourly systemd timer runs it (deploy/systemd/2drivescan-downloads.*).
 */
class SnapshotDownloads extends Command
{
    protected $signature = 'downloads:snapshot';

    protected $description = 'Store today\'s download counts from GitHub for the statistics page';

    public function handle(DownloadStats $stats): int
    {
        $summary = $stats->snapshot();
        if ($summary === null) {
            $this->error('GitHub could not be reached; no snapshot written.');

            return self::FAILURE;
        }

        $os = collect($summary['os'])->map(fn (int $n, string $name) => "{$name} {$n}")->implode(', ');
        $this->info("Installer downloads: {$summary['total']} ({$os}); macOS in-app updates: {$summary['mac_updates']}.");
        $this->line(count($summary['versions']).' releases, '.count($stats->history()).' days of history in '.$stats->historyPath());

        return self::SUCCESS;
    }
}
