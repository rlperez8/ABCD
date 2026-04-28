import { getPriceScale } from './geometry.js';

export const get_mid_price = (chartStateRef) => {
  const priceScale = getPriceScale(chartStateRef.current);
  const halfScreenHeight = chartStateRef.current.viewport.startingBaselineY / 2;

  if (!Number.isFinite(priceScale) || priceScale <= 0) {
    return 0;
  }

  return (chartStateRef.current.viewport.baselineY - halfScreenHeight) / priceScale;
};

export const get_pixel_location_of_a_price = (chartStateRef, price) => {
  return price * getPriceScale(chartStateRef.current);
};
