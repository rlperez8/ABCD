import pandas as pd
import numpy as np
from sqlalchemy import create_engine
import requests
import datetime
db_url = "mysql+pymysql://rperezkc:Nar8uto!@localhost:3306/abcd"
engine = create_engine(db_url)

def get_support_resistance(candles):

    # --- Clean and filter data ---
    candles['date'] = pd.to_datetime(candles['date']).dt.date
    candles.insert(5, 'adj_close', candles['close'])

    # Optional: save filtered data
    candles.to_csv("C:/Users/rpere/Desktop/abcd_local_v3/data.csv", index=False)

    # --- Parameters for SR scoring ---
    decay_per_tick = 0.01     # points lost per tick away
    range_pct = 0.05          # ±5% range for top SR selection
    reaction_tolerance = 0.01 # how close candle must touch/reverse

    # --- Create adaptive tick levels ---
    min_price = min(candles[['open','high','low','close']].min())
    max_price = max(candles[['open','high','low','close']].max())
    price_range = max_price - min_price

    # Dynamic tick interval (0.1% of range, min 0.01)
    tick_interval = max(0.01, round(price_range * 0.001, 2))
    print(f"Using tick interval: {tick_interval}")

    ticks = np.arange(min_price, max_price + tick_interval, tick_interval)

    # --- Initialize scores ---
    scores = pd.Series(0.0, index=ticks)

    # --- Reaction-only scoring ---
    for tick in ticks:
        for idx, row in candles.iterrows():
            # Only check support (low + reversal up)
            if row['low'] >= tick - reaction_tolerance and row['low'] <= tick + reaction_tolerance:
                if row['close'] > row['low']:  # candle reversed up
                    tick_dist = abs(tick - row['low']) / tick_interval
                    scores[tick] += max(0, 1 - tick_dist * decay_per_tick)

    # --- Create result DataFrame ---
    sr_df = pd.DataFrame({
        'Price': ticks,
        'Score': scores
    })

    # --- Add ±5% range columns ---
    sr_df['Lower'] = sr_df['Price'] * (1 - range_pct)
    sr_df['Upper'] = sr_df['Price'] * (1 + range_pct)

    # --- Pick top SR lines with ±5% removal ---
    top_sr_lines = []
    sr_copy = sr_df.copy()
    for _ in range(3):  # pick top 3 lines
        if sr_copy.empty:
            break
        top = sr_copy.loc[sr_copy['Score'].idxmax()]
        top_sr_lines.append(top)
        # remove ticks within ±5% of this top line
        lower = top['Price'] * (1 - range_pct)
        upper = top['Price'] * (1 + range_pct)
        sr_copy = sr_copy[(sr_copy['Price'] < lower) | (sr_copy['Price'] > upper)]

    top_sr_lines_df = pd.DataFrame(top_sr_lines).reset_index(drop=True)

    # --- Output ---
    print(top_sr_lines_df[['Price', 'Score', 'Lower', 'Upper']])

def get_candle_data_by_ticker(output_size, ticker):
        
        api_key = 'ZA9N4R1HE9ARIJ0S'
    
        timeframe = 'TIME_SERIES_DAILY'
        url = f'https://www.alphavantage.co/query?function={timeframe}&symbol={ticker}&outputsize={output_size}&apikey={api_key}'
        r = requests.get(url)
        data = r.json()

        if 'Error Message' in data or 'Time Series (Daily)' not in data:
            print('=================') 
            print('Error',ticker) 
            pass
        
        else:
            data = data['Time Series (Daily)']
            df = pd.DataFrame.from_dict(data, orient='index')
            df = df.reset_index()
            df = df.rename(columns={'index': 'date'})
            df['date'] = pd.to_datetime(df['date'])
            df['symbol'] = ticker
            df = df.rename(columns={
                '1. open': 'open',
                '2. high': 'high',
                '3. low': 'low',
                '4. close': 'close',
                '5. volume': 'volume'
            })


        return df

def get_distinct_symbols():
    distinct_symbols_query = f'SELECT DISTINCT symbol FROM abcd.candles'
    distinct_symbols = pd.read_sql_query(distinct_symbols_query, engine)
    return distinct_symbols


def get_stored_candles(symbol):
    query = '''
        SELECT * FROM abcd.candles
        WHERE symbol = %s
        AND YEAR(date) = %s
        
    '''

    candles = pd.read_sql_query(
        query,
        engine,
        params=(symbol, datetime.datetime.now().year)  # Correct single-item tuple
    )

    return candles

all_symbols = get_distinct_symbols()['symbol'].to_list()[0:20]


for symbol in all_symbols:

    candles_df = get_stored_candles(symbol)

    support_resistance_df = get_support_resistance(candles_df)

    print(support_resistance_df)