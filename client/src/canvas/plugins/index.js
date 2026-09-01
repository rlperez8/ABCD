export const createCanvasPlugin = (plugin) => ({
  layers: [],
  tools: [],
  panels: [],
  getOverlays: null,
  ...plugin,
});

