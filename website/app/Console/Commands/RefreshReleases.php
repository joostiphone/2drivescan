<?php

namespace App\Console\Commands;

use App\Services\Releases;
use Illuminate\Console\Command;

class RefreshReleases extends Command
{
    protected $signature = 'releases:refresh';

    protected $description = 'Forget cached GitHub release data and fetch the latest release';

    public function handle(Releases $releases): int
    {
        $releases->refresh();
        $latest = $releases->latest();
        if (! $latest) {
            $this->warn('GitHub could not be reached; downloads link to the releases page.');

            return self::SUCCESS;
        }
        $this->info("Latest release: {$latest['tag']} with ".count($latest['assets']).' installers.');

        return self::SUCCESS;
    }
}
