from sqlalchemy import text, create_engine
import pandas as pd
import json
import numpy as np
from sklearn.preprocessing import LabelEncoder
from sklearn.model_selection import train_test_split
from sklearn.ensemble import RandomForestClassifier
from sklearn.metrics import classification_report, precision_score, recall_score, f1_score, roc_auc_score, precision_recall_curve
import matplotlib.pyplot as plt
from imblearn.over_sampling import SMOTE
import joblib

# --- Database Access ---
def get_stored_table(table):
    db_url = "mysql+pymysql://rperezkc:Nar8uto!@localhost:3306/abcd"
    engine = create_engine(db_url)
    return pd.read_sql_query(text(f'SELECT * FROM {table}'), engine)

# --- Data Load ---
df1 = get_stored_table('pattern_ab')
df2 = get_stored_table('failed_pattern_ab')
df = pd.concat([df1, df2], ignore_index=True)

# --- Candle Feature Extraction ---
def extract_candle_features(candle_json):
    try:
        candles = json.loads(candle_json)
    except Exception:
        return pd.Series([np.nan]*6)

    if not candles:
        return pd.Series([np.nan]*6)

    opens = [c['open'] for c in candles]
    closes = [c['close'] for c in candles]
    highs = [c['high'] for c in candles]
    lows = [c['low'] for c in candles]
    volumes = [c['volume'] for c in candles]

    return pd.Series([
        len(candles),
        np.mean(np.array(highs) - np.array(lows)),
        np.mean(volumes),
        max(highs) - min(lows),
        np.mean(np.abs(np.array(closes) - np.array(opens))),
        sum(1 for o, c in zip(opens, closes) if c > o)
    ])

candle_features = df['candles'].apply(extract_candle_features)
candle_features.columns = [
    'candle_count', 'avg_candle_range', 'avg_volume', 'max_range', 'avg_body_size', 'bullish_candle_count'
]
df = pd.concat([df, candle_features], axis=1)

# --- Feature Engineering ---
df['pattern_A_duration'] = (pd.to_datetime(df['pattern_A_end_date']) - pd.to_datetime(df['pattern_A_start_date'])).dt.days
df['pattern_B_duration'] = (pd.to_datetime(df['pattern_B_end_date']) - pd.to_datetime(df['pattern_B_start_date'])).dt.days
df['pivot_AB_gap'] = (pd.to_datetime(df['pattern_B_pivot_date']) - pd.to_datetime(df['pattern_A_pivot_date'])).dt.days
df['high_diff'] = df['pattern_A_high'] - df['pattern_B_high']
df['low_diff'] = df['pattern_A_low'] - df['pattern_B_low']
df['high_low_ratio'] = df['pattern_A_high'] / df['pattern_B_low']

le_label = LabelEncoder()
df['label_enc'] = le_label.fit_transform(df['label'])

le_symbol = LabelEncoder()
df['trade_symbol_enc'] = le_symbol.fit_transform(df['trade_symbol'])

features = [
    'pattern_A_duration', 'pattern_B_duration', 'pivot_AB_gap',
    'pattern_A_high', 'pattern_A_low', 'pattern_B_high', 'pattern_B_low',
    'high_diff', 'low_diff', 'high_low_ratio', 'pattern_AB_bar_duration',
    'trade_symbol_enc'
] + candle_features.columns.tolist()

# --- Handle missing ---
X = df[features].fillna(0)
y = df['label_enc']

# --- Train-Test Split ---
X_train, X_test, y_train, y_test = train_test_split(
    X, y, test_size=0.2, random_state=42, stratify=y
)

# --- Apply SMOTE ---
smote = SMOTE(random_state=42)
X_train_res, y_train_res = smote.fit_resample(X_train, y_train)

print(f"Original training set size: {X_train.shape[0]}")
print(f"Resampled training set size: {X_train_res.shape[0]}")

# --- Train Model ---
model = RandomForestClassifier(n_estimators=100, random_state=42)
model.fit(X_train_res, y_train_res)

# --- Prediction ---
y_probs = model.predict_proba(X_test)
success_idx = list(le_label.classes_).index('Success')
success_probs = y_probs[:, success_idx]

# --- Threshold Tuning for 'Success' ---
threshold = 0.4  # Try values like 0.3, 0.5, 0.6 to tune
y_pred_thresh = (success_probs >= threshold).astype(int)
y_true_binary = (y_test == success_idx).astype(int)

print(f"\nThreshold: {threshold}")
print("Precision (Success):", precision_score(y_true_binary, y_pred_thresh))
print("Recall (Success):", recall_score(y_true_binary, y_pred_thresh))
print("F1-score (Success):", f1_score(y_true_binary, y_pred_thresh))
print("ROC AUC (Success):", roc_auc_score(y_true_binary, success_probs))

# --- Precision-Recall Curve Plot ---
precisions, recalls, thresholds = precision_recall_curve(y_true_binary, success_probs)
plt.figure(figsize=(8, 5))
plt.plot(thresholds, precisions[:-1], label='Precision')
plt.plot(thresholds, recalls[:-1], label='Recall')
plt.xlabel('Threshold')
plt.title('Precision & Recall vs Threshold for "Success"')
plt.legend()
plt.grid()
plt.show()

# --- Classification Report ---
y_pred = model.predict(X_test)
print("\nFull Classification Report:")
print(classification_report(y_test, y_pred, target_names=le_label.classes_))

# --- Feature Importances ---
importances = model.feature_importances_
feature_importance_df = pd.DataFrame({
    'feature': X.columns,
    'importance': importances
}).sort_values(by='importance', ascending=False)

print("\nTop Feature Importances:")
print(feature_importance_df.head(10))

plt.figure(figsize=(10, 6))
plt.barh(feature_importance_df['feature'].head(15), feature_importance_df['importance'].head(15))
plt.gca().invert_yaxis()
plt.xlabel('Importance')
plt.title('Top 15 Feature Importances')
plt.grid(True)
plt.show()

# --- Class Mappings ---
print("\nLabel classes:", dict(zip(le_label.classes_, le_label.transform(le_label.classes_))))
print("Trade symbols:", dict(zip(le_symbol.classes_, le_symbol.transform(le_symbol.classes_))))


# --- Train Model ---
model = RandomForestClassifier(n_estimators=100, random_state=42)
model.fit(X_train_res, y_train_res)

# Save model and encoders
joblib.dump(model, 'random_forest_model.joblib')
joblib.dump(le_label, 'label_encoder.joblib')
joblib.dump(le_symbol, 'symbol_encoder.joblib')