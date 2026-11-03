<?php

namespace Tests\Unit;

use App\Services\Changelog;
use PHPUnit\Framework\TestCase;

class ChangelogTest extends TestCase
{
    public function test_parser_reads_versions_dates_and_markdown(): void
    {
        $entries = (new Changelog)->parse("# Changelog\n\nIntro\n\n## 1.2.0 – 2026-12-01\n\n- **New** thing\n\n## [1.1.0]\n\n- Older\n");

        $this->assertCount(2, $entries);
        $this->assertSame('1.2.0', $entries[0]['version']);
        $this->assertSame('2026-12-01', $entries[0]['date']);
        $this->assertStringContainsString('<strong>New</strong>', $entries[0]['html']);
        $this->assertSame('1.1.0', $entries[1]['version']);
        $this->assertNull($entries[1]['date']);
    }

    public function test_html_in_the_changelog_is_escaped(): void
    {
        $entries = (new Changelog)->parse("## 1.0.0\n\n<script>alert(1)</script>\n");

        $this->assertStringNotContainsString('<script>', $entries[0]['html']);
    }
}
