from storage import *
import traceback



def run_strategy(ticker):
    try: 
        
        candles = DataBase().get_stored_candles('candles', ticker)
       
        candles = candles[['date','open','high','low','close','volume']]
        candles['date'] = pd.to_datetime(candles['date']).dt.date
        # candles = candles[candles['date']>= pd.to_datetime('2020-01-01').date()]
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

    # # Get Tickers
    tickers = alpha.get_listing_status()['symbol'].to_list()[105:]
    # print(tickers)

    # # # Load Candles
    for ticker in tickers:
        try:
            alpha.load_single_symbol_candle_data('full', ticker)
            run_strategy(ticker)
            print('Scanning ',ticker)
        except: 
            pass


    # # Pull Loaded Candle Tickers
    # my_tickers = db.get_distinct_symbols('candles','symbol')['symbol'].to_list()[70:100]
    # # Run Strategy
    # for symbol in my_tickers:
    #     run_strategy(symbol)



main()