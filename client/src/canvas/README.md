# Canvas

This folder is the staging area for the reusable candle canvas.

The goal is to separate chart infrastructure from trading-specific features:

- `core` owns candle rendering, viewport state, scales, and the chart surface.
- `overlays` owns generic chart-world objects such as lines, markers, labels, zones, and paths.
- `plugins` owns the plugin contract and plugin host helpers.
- `tools` owns shared interaction tools such as crosshair, selection, and drawing input.
- `theme` owns reusable canvas colors, spacing, and drawing tokens.

Trading-specific concepts like ABCD patterns, trend model scores, NinjaTrader state, strategies, and server API calls should stay outside the canvas core. They can later become plugins or app-level adapters that convert domain data into generic overlays.

Plugins should describe chart objects using candle/time/price anchors. The canvas converts those anchors into screen pixels during render.

