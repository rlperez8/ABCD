from storage import *
import traceback
import sys


def run_strategy(ticker):
    try:
        candles = DataBase().get_stored_candles('candles', ticker)

        # If not enough candles, exit this function
        if candles.shape[0] < 365:
            print('candles.shape[0] < 365')
            with open('empty_symbols.txt', 'a') as f:
                f.write(ticker + '\n')
            return
     
        db_url = "mysql+pymysql://rperezkc:Nar8uto!@localhost:3306/abcd"
        engine = create_engine(db_url)

        query = '''
                select * 
                from support_and_resistance
                where symbol = %s
            '''
        df = pd.read_sql_query(query, engine, params=(ticker,))
        snr_price = df.iloc[0].price

        candles = candles[['date','open','high','low','close','volume']]
        candles['date'] = pd.to_datetime(candles['date']).dt.date
        # candles = candles[candles['date'] >= pd.to_datetime('2024-12-01').date()]
        candles.insert(5, 'adj_close', candles['close'])

        if candles.empty:
            print('candles.empty:')
            with open('candles df empty.txt', 'a') as f:
                f.write(ticker + '\n')
            return
      

        candles.to_csv("data.csv", index=False)
        btFeed = Backtrader().prepareDataForStrategy(candles, 'data.csv')
        cerebro = bt.Cerebro(writer=False)
        cerebro.adddata(btFeed)
        cerebro.addstrategy(ABCD, 
                pattern_A = [],
                pattern_AB = [],
                pattern_ABC = [],
                pattern_ABCD = [],
                stockName=ticker, 
                snr_price=snr_price
            )
        cerebro.addanalyzer(bt.analyzers.TradeAnalyzer, _name="ta")
        cerebro.addanalyzer(bt.analyzers.SQN, _name="sqn")

        print('candle df size',len(candles))
        cerebro.run()

    except Exception as e:
        print('Failed:', e)
        traceback.print_exc()
        sys.exit()  # exits the entire program if an exception occurs

def abcd_patterns():
    db = DataBase()

    pattern_abcd_symbols =  db.get_distinct_symbols('pattern_abcd','symbol')['symbol'].tolist()
    candles_symbols =  db.get_distinct_symbols('candles','symbol')['symbol'].tolist()

    missing_symbols = list(set(candles_symbols) - set(pattern_abcd_symbols))

    for index, ticker in enumerate(candles_symbols):
        try:
            print('============================')
            print(index, len(candles_symbols),ticker)
            run_strategy("A")
            sys.exit()
                
        except Exception as e: 
            print(f"Error processing {ticker}: {e}")
            pass


    print('snr_symbols:', len(pattern_abcd_symbols))
    print('candles_symbols:', len(candles_symbols))

def load_candles():

    alpha = AlphaVantage()
    tickers = alpha.get_listing_status()['symbol'].to_list()[2300:]
    db = DataBase()

    for index, ticker in enumerate(tickers):
        try:
            df = alpha.load_single_symbol_candle_data('full', ticker)

            if not df.empty:
            
                df['volume'] = pd.to_numeric(df['volume'], errors='coerce')
                avg_volume = df['volume'].mean()

                # Skip illiquid stocks
                if avg_volume < 500_000:
                    continue
        
            try:
                db.insert_data(df, 'candles')
                print(f"Ticker: {ticker}, Avg Volume: {avg_volume:.0f}", 'Index: ', index + 2300 )
            except: 
                pass
         
        except Exception as e: 
            print(f"Error processing {ticker}: {e}")

def main():
    abcd_patterns()


main()

# OTC
# CAUD
# CUBT EFTR