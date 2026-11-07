<?php

namespace App\Support;

use Illuminate\Support\Carbon;

/**
 * Geometry for a small line chart drawn as inline SVG on the server (no
 * chart library, nothing loaded from elsewhere). Time runs along the x axis
 * in proportion to the dates; the y axis starts at zero.
 */
class LineChart
{
    /**
     * @param  array<string, int>  $series  date (Y-m-d) => value, oldest first
     * @return array<string, mixed>|null null with fewer than two points
     */
    public static function build(array $series, int $width = 960, int $height = 300, ?string $locale = null): ?array
    {
        if (count($series) < 2) {
            return null;
        }
        ksort($series);
        $step = self::niceStep(max(1, max($series)) / 4);
        $max = $step * 4;
        // Room for the longest y label at the largest font the page uses (phones).
        $left = 20.0 + 18 * strlen(number_format($max));
        [$right, $top, $bottom] = [$width - 24.0, 18.0, $height - 48.0];

        $dates = array_keys($series);
        $t0 = Carbon::parse($dates[0])->getTimestamp();
        $t1 = Carbon::parse(end($dates))->getTimestamp();
        $span = max(1, $t1 - $t0);

        $x = fn (string $date) => $left + (Carbon::parse($date)->getTimestamp() - $t0) / $span * ($right - $left);
        $y = fn (int $value) => $bottom - $value / $max * ($bottom - $top);
        $f = fn (float $n) => rtrim(rtrim(number_format($n, 1, '.', ''), '0'), '.');

        $points = [];
        foreach ($series as $date => $value) {
            $points[] = ['x' => $f($x((string) $date)), 'y' => $f($y((int) $value)), 'date' => (string) $date, 'value' => (int) $value];
        }
        $line = 'M'.implode(' L', array_map(fn ($p) => $p['x'].' '.$p['y'], $points));
        $area = $line.' L'.end($points)['x'].' '.$f($bottom).' L'.$points[0]['x'].' '.$f($bottom).' Z';

        $yticks = [];
        for ($i = 0; $i <= 4; $i++) {
            $yticks[] = ['y' => $f($y($step * $i)), 'value' => $step * $i];
        }

        // At most six date labels, spread over the range.
        $count = count($dates);
        $labels = $count <= 6 ? range(0, $count - 1) : array_unique(array_map(fn ($i) => (int) round($i * ($count - 1) / 5), range(0, 5)));
        $xticks = [];
        foreach ($labels as $i) {
            $date = Carbon::parse($dates[$i])->locale($locale ?? app()->getLocale());
            $xticks[] = ['x' => $f($x($dates[$i])), 'label' => $date->isoFormat('D MMM')];
        }

        return [
            'width' => $width, 'height' => $height, 'line' => $line, 'area' => $area, 'points' => $points,
            'yticks' => $yticks, 'xticks' => $xticks,
            'left' => $left, 'right' => $right, 'top' => $top, 'bottom' => $bottom,
        ];
    }

    /** 1, 2 or 5 times a power of ten, at least $raw and at least 1. */
    public static function niceStep(float $raw): int
    {
        $raw = max(1.0, $raw);
        $pow = 10 ** floor(log10($raw));
        foreach ([1, 2, 5, 10] as $m) {
            if ($m * $pow >= $raw) {
                return (int) ($m * $pow);
            }
        }

        return (int) (10 * $pow);
    }
}
