<?php

namespace App\Services;

use Illuminate\Support\Str;

/**
 * Renders CHANGELOG.md (or CHANGELOG.nl.md for Dutch) from the repository
 * root into a list of versions.
 */
class Changelog
{
    /**
     * @return list<array{version: string, date: ?string, html: string}>
     */
    public function entries(string $locale): array
    {
        $root = rtrim(config('site.repo_path'), '/');
        $candidates = $locale === 'en'
            ? [$root.'/CHANGELOG.md']
            : [$root.'/CHANGELOG.'.$locale.'.md', $root.'/CHANGELOG.md'];

        foreach ($candidates as $file) {
            if (is_readable($file)) {
                return $this->parse((string) file_get_contents($file));
            }
        }

        return [];
    }

    /**
     * Splits on "## <version> – <date>" headings.
     *
     * @return list<array{version: string, date: ?string, html: string}>
     */
    public function parse(string $markdown): array
    {
        $parts = preg_split('/^##\s+/m', $markdown);
        array_shift($parts); // Title and intro before the first version.

        $entries = [];
        foreach ($parts as $part) {
            [$heading, $body] = array_pad(explode("\n", $part, 2), 2, '');
            preg_match('/^\[?v?([0-9][^\]\s]*)\]?\s*(?:[–—-]\s*(\d{4}-\d{2}-\d{2}))?/u', trim($heading), $m);
            $entries[] = [
                'version' => $m[1] ?? trim($heading),
                'date' => $m[2] ?? null,
                'html' => Str::markdown(trim($body), ['html_input' => 'escape', 'allow_unsafe_links' => false]),
            ];
        }

        return $entries;
    }
}
