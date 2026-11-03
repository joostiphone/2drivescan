<?php

namespace App\Console\Commands;

use Illuminate\Console\Command;
use Illuminate\Support\Facades\File;

/**
 * Copies files the website shares with the app from the repository root:
 * the app UI (served as the live demo), screenshots and icons. Run on every
 * deploy so the site always shows the current app.
 */
class SyncAssets extends Command
{
    protected $signature = 'site:sync';

    protected $description = 'Copy the app UI, screenshots and icons from the repository into public/';

    public function handle(): int
    {
        $repo = rtrim(config('site.repo_path'), '/');
        $public = public_path();

        if (! is_dir($repo.'/ui')) {
            $this->error("App UI not found in {$repo}/ui (set SITE_REPO_PATH).");

            return self::FAILURE;
        }

        File::deleteDirectory($public.'/demo');
        File::copyDirectory($repo.'/ui', $public.'/demo');
        $this->line('demo     ← ui/');

        File::ensureDirectoryExists($public.'/img/screens');
        foreach (File::glob($repo.'/docs/screenshots/*.png') as $shot) {
            File::copy($shot, $public.'/img/screens/'.basename($shot));
        }
        $this->line('img      ← docs/screenshots/');

        $icons = [
            'assets/icon-1024.png' => 'img/icon-1024.png',
            'src-tauri/icons/128x128@2x.png' => 'img/logo.png',
            'src-tauri/icons/32x32.png' => 'favicon.png',
            'src-tauri/icons/icon.ico' => 'favicon.ico',
        ];
        foreach ($icons as $from => $to) {
            if (is_file($repo.'/'.$from)) {
                File::copy($repo.'/'.$from, $public.'/'.$to);
            }
        }
        $this->line('icons    ← assets/, src-tauri/icons/');

        $this->info('Assets synced.');

        return self::SUCCESS;
    }
}
