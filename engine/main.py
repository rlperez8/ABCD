from storage import *
import traceback
import sys


def run_strategy(ticker):
    try: 
        
        candles = DataBase().get_stored_candles('candles', ticker)
       
        candles = candles[['date','open','high','low','close','volume']]
        candles['date'] = pd.to_datetime(candles['date']).dt.date
        candles = candles[candles['date']>= pd.to_datetime('2024-12-01').date()]
        candles.insert(5, 'adj_close', candles['close'])
        candles.to_csv("data.csv",index=False)
        btFeed = Backtrader().prepareDataForStrategy(candles, 'data.csv')
        cerebro = bt.Cerebro(writer=False)
        cerebro.adddata(btFeed)
        cerebro.addstrategy(ABCD, 
                pattern_A = [],
                pattern_AB = [],
                pattern_ABC = [],
                pattern_ABCD = [],
                stockName=ticker, 
            )
        cerebro.addanalyzer(bt.analyzers.TradeAnalyzer, _name="ta")
        cerebro.addanalyzer(bt.analyzers.SQN, _name="sqn")
        cerebro.run()
    except Exception as e: 
        print('Failed:', e)
        traceback.print_exc()  
        pass

def main():
    alpha = AlphaVantage()
    db = DataBase()

    df = alpha.load_single_symbol_candle_data('full', 'ABUS')
    df['volume'] = pd.to_numeric(df['volume'], errors='coerce')
    avg_volume = df['volume'].mean()
    print(df)
    run_strategy('ABUS')
    sys.exit()

    # Get tickers (first 100)
    tickers = alpha.get_listing_status()['symbol'].to_list()[7975:]

    
    for index, ticker in enumerate(tickers):
        try:
            df = alpha.load_single_symbol_candle_data('full', ticker)
            df['volume'] = pd.to_numeric(df['volume'], errors='coerce')
            avg_volume = df['volume'].mean()

            print(f"Ticker: {ticker}, Avg Volume: {avg_volume:.0f}", 'Index: ', index + 7975 )

            # Skip illiquid stocks
            if avg_volume < 500_000:
                continue
            
            try:
                db.insert_data(df, 'candles')
            except: 
                pass
            run_strategy(ticker)
                
        except Exception as e: 
            print(f"Error processing {ticker}: {e}")

            sys.exit()



main()