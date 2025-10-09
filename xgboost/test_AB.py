import pandas as pd
import numpy as np
import json
import pickle

# Load your saved model and label encoders
with open('le_symbol.pkl', 'rb') as f:
    le_symbol = pickle.load(f)

with open('le_label.pkl', 'rb') as f:
    le_label = pickle.load(f)

with open('xgb_model.pkl', 'rb') as f:
    model = pickle.load(f)


# Example single AB pattern data
single_ab = {
    'candles': json.dumps([
        {'open': 96, 'high': 101, 'low': 94, 'close': 99, 'volume': 1000},
        {'open': 99, 'high': 102, 'low': 95, 'close': 97, 'volume': 1100},
        {'open': 97, 'high': 99, 'low': 90, 'close': 92, 'volume': 900}
    ]),
    'pattern_A_start_date': '2025-05-01',
    'pattern_A_end_date': '2025-05-02',
    'pattern_B_start_date': '2025-05-03',
    'pattern_B_end_date': '2025-05-04',
    'pattern_A_pivot_date': '2025-05-01',
    'pattern_B_pivot_date': '2025-05-03',
    'pattern_A_high': 100,
    'pattern_A_low': 95,
    'pattern_B_high': 98,
    'pattern_B_low': 90,
    'pattern_AB_bar_duration': 3,
    'trade_symbol': 'A',
    # 'label': 'Success'  # omit for prediction
}

# Convert to DataFrame
df_single = pd.DataFrame([single_ab])


# Feature extraction from candles
def extract_candle_features(candle_json):
    candles = json.loads(candle_json)
    candle_count = len(candles)
    if candle_count == 0:
        return pd.Series({
            'candle_count': 0,
            'avg_candle_range': 0,
            'avg_volume': 0,
            'max_range': 0,
            'avg_body_size': 0,
            'bullish_candle_count': 0
        })
    ranges = [c['high'] - c['low'] for c in candles]
    volumes = [c.get('volume', 0) for c in candles]
    bodies = [abs(c['close'] - c['open']) for c in candles]
    bullish_count = sum(1 for c in candles if c['close'] > c['open'])

    return pd.Series({
        'candle_count': candle_count,
        'avg_candle_range': np.mean(ranges),
        'avg_volume': np.mean(volumes),
        'max_range': max(ranges),
        'avg_body_size': np.mean(bodies),
        'bullish_candle_count': bullish_count
    })


# Apply candle features
candle_features = df_single['candles'].apply(extract_candle_features)
df_single = pd.concat([df_single, candle_features], axis=1)


# Calculate durations and differences
df_single['pattern_A_duration'] = (pd.to_datetime(df_single['pattern_A_end_date']) - pd.to_datetime(df_single['pattern_A_start_date'])).dt.days
df_single['pattern_B_duration'] = (pd.to_datetime(df_single['pattern_B_end_date']) - pd.to_datetime(df_single['pattern_B_start_date'])).dt.days
df_single['pivot_AB_gap'] = (pd.to_datetime(df_single['pattern_B_pivot_date']) - pd.to_datetime(df_single['pattern_A_pivot_date'])).dt.days
df_single['high_diff'] = df_single['pattern_A_high'] - df_single['pattern_B_high']
df_single['low_diff'] = df_single['pattern_A_low'] - df_single['pattern_B_low']
df_single['high_low_ratio'] = df_single['pattern_A_high'] / df_single['pattern_B_low']

# Encode trade_symbol using pre-fitted LabelEncoder
df_single['trade_symbol_enc'] = le_symbol.transform(df_single['trade_symbol'])

# Features for the model (make sure these match your trained model's expected features)
features = [
    'pattern_A_duration', 'pattern_B_duration', 'pivot_AB_gap',
    'pattern_A_high', 'pattern_A_low', 'pattern_B_high', 'pattern_B_low',
    'high_diff', 'low_diff', 'high_low_ratio', 'pattern_AB_bar_duration', 'trade_symbol_enc',
    'candle_count', 'avg_candle_range', 'avg_volume', 'max_range', 'avg_body_size', 'bullish_candle_count'
]

X_single = df_single[features].fillna(0)

# Predict
predicted_label_enc = model.predict(X_single)[0]
predicted_label = le_label.inverse_transform([predicted_label_enc])[0]

print(f"Predicted class: {predicted_label}")
