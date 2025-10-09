import backtrader as bt
import requests
from sqlalchemy import create_engine, text
from sqlalchemy import create_engine, Column, Integer, String, MetaData, Table, Date, update,exc
import csv
import requests
import pandas as pd
from backtrader.feeds.yahoo import YahooFinanceCSV
import datetime
import numpy as np
from statistics import mean
import uuid
import concurrent.futures
import sys
import psycopg2
import json
allResults      = []
chartData       = []
allTrades       = []
a_pivots = []

# PATTERNS
pattern_a = []
pattern_ab = []
pattern_abc = []
pattern_abcd = []

class DataBase:

    def __init__(self) -> None:
     
        self.db_url = "mysql+pymysql://rperezkc:Nar8uto!@localhost:3306/abcd"
        self.engine = create_engine(self.db_url)

    def update_today_candle(self):
        active_symbols = DataBase().get_stored_table('listing_status')
        active_symbols = active_symbols['symbol'].to_list()

        # print(active_symbols)

        for i,symbol in enumerate(active_symbols):


            # Assuming AlphaVantage() returns a DataFrame with a 'date' column and floating-point columns
            candles = AlphaVantage().get_single_symbol_candles('compact', symbol)

            # Convert the first row to DataFrame
            df = pd.DataFrame(candles.iloc[1]).transpose()
    
            # Convert 'date' column to datetime   
            df['date'] = pd.to_datetime(df['date'])

            # Extract just the date part
            df['date'] = df['date'].dt.date

            # Convert 'date' to string if needed
            df['date'] = df['date'].astype(str)

            # Define the columns that should be rounded
            float_columns = ['open', 'high', 'low', 'close']

            # Ensure these columns are of type float
            for col in float_columns:
                df[col] = pd.to_numeric(df[col], errors='coerce')  # Convert to numeric, coerce errors to NaN

            # Round the floating-point columns to 2 decimal places
            df[float_columns] = df[float_columns].round(2)

            date_value = df.loc[1]['date']

            df2 = DataBase().get_stock_data(symbol)
            df['date'] = df['date'].astype(str)
            df2['date'] = df2['date'].astype(str)

            exists = date_value in df2['date'].values

            if exists == False:
                print(f'Updated {symbol}', i, len(active_symbols))
                # print(df)
                DataBase().insert_data(df, 'candles')
                print('=====================')
            elif exists == True:
                print(f'{symbol} is up to date', i, len(active_symbols))
    
    def get_stock_data(self, symbol):
        # Construct the SQL query with the correct syntax
        stored_accounts_query = f'''
            SELECT * FROM candles
            WHERE symbol = '{symbol}'
        '''
        
        # Execute the query and fetch data into a DataFrame
        stored_accounts = pd.read_sql_query(stored_accounts_query, self.engine)

        return stored_accounts
  
    def update_inactive_symbols(self):
        listing_status = DataBase().get_stored_table('listing_status')
        symbols = listing_status['symbol'].to_list()


        not_current = []
        for i, symbol in enumerate(symbols):

            print(i,len(symbols))
            candles = DataBase().get_most_recent_candle_for_symbol('candles',symbol)
            date = str(candles.loc[0, 'date'])
        
            if '2024-08-02' != date:
                print(candles)
                not_current.append(symbol)

        DataBase().update_symbol_status_to_inactive('listing_status',not_current)
    
    def update_symbol_status_to_inactive(self, table_name, symbol_list):
        # Convert symbols in the list to a format that can be used in SQL
        symbol_list_str = ", ".join([f"'{symbol}'" for symbol in symbol_list])
        
        # Construct the SQL update query
        update_query = f"""
        UPDATE {table_name}
        SET status = 'inactive'
        WHERE symbol IN ({symbol_list_str})
        """
        
        # Execute the update query using text
        with self.engine.connect() as conn:
            result = conn.execute(text(update_query))
            conn.commit()
            print(f"{result.rowcount} rows updated.")

    def get_most_recent_candle_for_symbol(self, table_name, symbol):
        query = f"""
        SELECT * FROM {table_name}
        WHERE symbol = '{symbol}'
        ORDER BY date DESC
        LIMIT 1
        """
        df = pd.read_sql_query(query, self.engine)
        return df
    
    def get_candles_for_symbol(self, table_name, symbol):
        query = f"SELECT * FROM {table_name} WHERE symbol = '{symbol}'"
        df = pd.read_sql_query(query, self.engine)

        return df
    
    def get_distinct_symbols(self, table, symbol_column):
        distinct_symbols_query = f'SELECT DISTINCT {symbol_column} FROM {table}'
        distinct_symbols = pd.read_sql_query(distinct_symbols_query, self.engine)
        return distinct_symbols
    
    def get_stored_table(self, table):

        stored_accounts_query = text(f'''
            SELECT * FROM {table}
      


        ''')
        stored_accounts = pd.read_sql_query(stored_accounts_query, self.engine)

        return stored_accounts

    def get_stored_candles(self, table, symbol):
        stored_accounts_query = text(f'''
            SELECT * FROM {table}
            WHERE symbol = :symbol
        ''')

        stored_accounts = pd.read_sql_query(
            stored_accounts_query,
            self.engine,
            params={"symbol": symbol}
        )

        return stored_accounts

    def delete_data(self, table):

        metadata = MetaData()
        try:
            table = Table(table, metadata, autoload_with=self.engine)

            with self.engine.connect() as connection:

                connection.execute(table.delete())
                connection.commit()

        except exc.SQLAlchemyError as e:
            print(f"SQLAlchemy error: {e}")
        except Exception as e:
            print(f"Error deleting rows from {table} table: {e}")

    def insert_data(self, data,table):

        # Insert DataFrame into the database table
        try:
            data.to_sql(table, self.engine, index=False, if_exists='append')
       
    
        except Exception as e:
            print(f"Error inserting data: {e}")
   
    def test_connection(self):
        try:
            with self.engine.connect() as connection:
                # Use `text` to handle SQL query
                result = connection.execute(text("SELECT 1"))
                one = result.fetchone()[0]
                print(f"Connection successful: {one}")
        except Exception as e:
            print(f"Connection failed: {e}")

    def get_latest_candle_dates(self):

        all_candles = DataBase().get_stored_table('candles')
        print(all_candles)

    def update_symbol_status(self, table_name, symbol):
        metadata = MetaData()
        table = Table(table_name, metadata, autoload_with=self.engine)
        
        # Construct the update statement
        update_stmt = (
            update(table)
            .where(table.c.symbol == symbol)
            .values(bugged=True)
        )
        
        try:
            # Execute the update statement
            with self.engine.connect() as connection:
                result = connection.execute(update_stmt)
                connection.commit()
            print(f"Symbol '{symbol}' status updated to False.")
            
        except Exception as e:
            print(f"Error occurred: {str(e)}")

    def connect_to_azure_db(self):
        host = "abcdserver.postgres.database.azure.com"
        port = "5432"
        database = "postgres"  # e.g., 'postgres' or your actual DB name
        user = "rperezkc35"
        password = "Nar8uto!"

        try:
            connection = psycopg2.connect(
                host=host,
                port=port,
                database=database,
                user=user,
                password=password,
                sslmode='require'  # Azure PostgreSQL requires SSL connection
            )

            print("Connected to the database successfully!")

            # Create a cursor object
            cursor = connection.cursor()

            # Run a query (example: show all tables)
            cursor.execute("SELECT table_name FROM information_schema.tables WHERE table_schema = 'public';")
            tables = cursor.fetchall()

            print("Tables in the database:")
            for table in tables:
                print(table)

        except Exception as e:
            print(f"Error: {e}")
        finally:
            # Close the cursor and connection if open
            if connection:
                cursor.close()
                connection.close()
                print("Connection closed.")

class AlphaVantage:

    def __init__(self) -> None:
        
        self.api_key = '10NSLCA2J0HLW7HG'
    
    def validate_candle_activity():
        all_candles = DataBase().get_stored_table('candles')
        all_symbols = all_candles['symbol'].to_list()
        yesterdays_candles = all_candles[all_candles['date'].astype(str) == '2024-08-16']
        symbols = yesterdays_candles['symbol'].to_list()

        diff = set(all_symbols) - set(symbols)

        for each in diff:
            DataBase().update_symbol_status('listing_status',each)

    def load_all_symbol_candle_data(self):

        listing_status = DataBase().get_stored_table('listing_status')
        symbols = listing_status['symbol'].to_list()

        for i, symbol in enumerate(symbols):
            print(symbol, i, len(symbols))
            AlphaVantage().load_single_symbol_candle_data('full',symbol)
    
    def get_single_symbol_candle_data(self, output_size, ticker):
        api_key = self.api_key
    
        timeframe = 'TIME_SERIES_DAILY'
        url = f'https://www.alphavantage.co/query?function={timeframe}&symbol={ticker}&outputsize={output_size}&apikey={api_key}'
        r = requests.get(url)
        data = r.json()

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

    def load_single_symbol_candle_data(self, output_size, ticker):
        
        api_key = self.api_key
    
        timeframe = 'TIME_SERIES_DAILY'
        url = f'https://www.alphavantage.co/query?function={timeframe}&symbol={ticker}&outputsize={output_size}&apikey={api_key}'
        r = requests.get(url)
        data = r.json()


        if 'Error Message' in data or 'Time Series (Daily)' not in data:
            print('=================') 
            print('Error',ticker) 
            DataBase().update_symbol_status('listing_status',ticker)
            return pd.DataFrame() 
        
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

            DataBase().insert_data(df, 'candles')

    def get_all_symbol_candle_data_batch(self, output_size, tickers, max_workers=5):
        with concurrent.futures.ThreadPoolExecutor(max_workers=max_workers) as executor:
            futures = {executor.submit(self.load_single_symbol_candle_data, output_size, ticker): ticker for ticker in tickers}
            results = []
            for future in concurrent.futures.as_completed(futures):
                ticker = futures[future]
                try:
                    data = future.result()
                    results.append(data)
                except Exception as e:
                    print(f"{ticker} generated an exception: {e}")
        return pd.concat(results, ignore_index=True) if results else pd.DataFrame()
    
    def get_listing_status(self):
        
        CSV_URL = 'https://www.alphavantage.co/query?function=LISTING_STATUS&apikey=10NSLCA2J0HLW7HG'

        with requests.Session() as s:
            download = s.get(CSV_URL)
            decoded_content = download.content.decode('utf-8')
            cr = csv.reader(decoded_content.splitlines(), delimiter=',')
            my_list = list(cr)

            df = pd.DataFrame(my_list)
            df = df.drop(0)
            df = df.rename(columns={
                0:'symbol',
                1:'name',
                2:'exchange',
                3:'assetType',
                4:'ipoDate',
                5:'delistingDate',
                6:'status',

                
                })
        

        return  df

    def update_missing_candles(symbol):

        ticker_symbols = DataBase().get_stored_table('listing_status')
        ticker_symbols_list = ticker_symbols[ticker_symbols['bugged'] == False]['symbol'].to_list()

        for symbol in ticker_symbols_list:

            # Get the latest date of stored candle data
            candle_data = DataBase().get_candles_for_symbol('candles',symbol)
            latest_date = candle_data[candle_data['date'] == candle_data['date'].max()]
            latest_date = latest_date['date'].iloc[0].strftime('%Y-%m-%d')

            # Get the latest date of alpha candle data
            alph_candle_data = AlphaVantage().get_single_symbol_candle_data('full',symbol)
            latest_date_alpha = alph_candle_data[alph_candle_data['date'] == alph_candle_data['date'].max()]
            latest_date_alpha = latest_date_alpha['date'].iloc[0].strftime('%Y-%m-%d')

            # Get missing candles
            def is_weekend(date_str):
                date = datetime.strptime(date_str, '%Y-%m-%d')  # Convert string to datetime
                return date.weekday() >= 5 
            today = datetime.today().date()
            date_range = pd.date_range(start=latest_date, end=today)
            date_list = date_range.strftime('%Y-%m-%d').tolist()
            weekdays = [date for date in date_list if not is_weekend(date)]
            del weekdays[0]
            filtered_df = alph_candle_data[alph_candle_data['date'].isin(weekdays)]

            DataBase().insert_data(filtered_df, 'candles')

class Backtrader:

    def prepareDataForStrategy(self, df,datapath):

        startYear,startMonth,startDays,endingYear,endingMonth,endingDays = Backtrader().get_first_and_last_data_of_DF(df)

        btFeed = Backtrader().csv_into_btfeeds(datapath,startYear,startMonth,startDays,endingYear,endingMonth,endingDays)
        
    
        return btFeed
    
    def get_first_and_last_data_of_DF(self, df):

        # print(df.iloc[0])
        earliestDate  = str(df.iloc[0]['date']).split('-')
   
        startYear     = earliestDate[0]
        startMonth    = earliestDate[1]
        startDays     = earliestDate[2]
        
        latestDate    = str(df.iloc[-1]['date']).split('-')
        endingYear    = latestDate[0]
        endingMonth   = latestDate[1]
        endingDays    = latestDate[2]

        return startYear, startMonth, startDays, endingYear, endingMonth, endingDays

    def csv_into_btfeeds(self, datapath,startYear,startMonth,startDays,endingYear,endingMonth,endingDays):

        data = bt.feeds.YahooFinanceCSVData(
            dataname=datapath,
            fromdate=datetime.datetime(int(startYear),int(startMonth), int(startDays)),
            todate=datetime.datetime(int(endingYear), int(endingMonth), int(endingDays)),
            adjclose=False)
        
        # with open(datapath, 'r') as f:
        #     for _ in range(10):  # Change to more if needed
        #         print(f.readline().strip())
        return data
    
class StrategyTools:

    def load_a_patterns(self, a_patterns):
        list_of_pattern_A = []
        for each in a_patterns:
            a_pat = {
                'pattern_A_start_date':each.pattern_A_start_date,
                'pattern_A_pivot_date':each.pattern_A_pivot_date,
                'pattern_A_end_date':each.pattern_A_end_date,
                'pattern_A_high':each.pattern_A_high,
                'pattern_A_close':each.pattern_A_close,
                'pattern_A_open':each.pattern_A_open,
                'pattern_A_low':each.pattern_A_low,
                'trade_symbol':each.trade_symbol
            }
            list_of_pattern_A.append(a_pat)
        
        pattern_A_df = pd.DataFrame(list_of_pattern_A)
        DataBase().insert_data(pattern_A_df,'pattern_a')

    def load_ab_patterns(self, ab_patterns):

        try: 
            list_of_pattern_AB = []
            for each in ab_patterns:
                ab_pat = {
                    'id': each.id,
                    'pattern_A_start_date': each.pattern_A_start_date,
                    'pattern_A_pivot_date': each.pattern_A_pivot_date,
                    'pattern_A_end_date': each.pattern_A_end_date,
                    'pattern_A_high': each.pattern_A_high,
                    'pattern_A_close': each.pattern_A_close,
                    'pattern_A_open': each.pattern_A_open,
                    'pattern_A_low': each.pattern_A_low,
                    'pattern_B_start_date': each.pattern_B_start_date,
                    'pattern_B_pivot_date': each.pattern_B_pivot_date,
                    'pattern_B_end_date': each.pattern_B_end_date,
                    'pattern_B_high': each.pattern_B_high,
                    'pattern_B_close': each.pattern_B_close,
                    'pattern_B_open': each.pattern_B_open,
                    'pattern_B_low': each.pattern_B_low,
                    'trade_symbol': each.trade_symbol,
                    'pattern_AB_bar_duration': each.pattern_AB_bar_duration,
                    'label': each.label,
                    # 'candles': each.candles.to_json(orient='records')
                }
                list_of_pattern_AB.append(ab_pat)

            pattern_AB_df = pd.DataFrame(list_of_pattern_AB)
            DataBase().insert_data(pattern_AB_df, 'pattern_ab')

        except Exception as error:
            print(error)


    def load_ab_failed_patterns(self, ab_patterns):
        list_of_pattern_AB = []
        for each in ab_patterns:
            ab_pat = {
                'pattern_A_start_date':each.pattern_A_start_date,
                'pattern_A_pivot_date':each.pattern_A_pivot_date,
                'pattern_A_end_date':each.pattern_A_end_date,
                'pattern_A_high':each.pattern_A_high,
                'pattern_A_close':each.pattern_A_close,
                'pattern_A_open':each.pattern_A_open,
                'pattern_A_low':each.pattern_A_low,
                'pattern_B_start_date':each.pattern_B_start_date,
                'pattern_B_pivot_date':each.pattern_B_pivot_date,
                'pattern_B_end_date':each.pattern_B_end_date,
                'pattern_B_high':each.pattern_B_high,
                'pattern_B_close':each.pattern_B_close,
                'pattern_B_open':each.pattern_B_open,
                'pattern_B_low':each.pattern_B_low,
                'trade_symbol':each.trade_symbol,
                'pattern_AB_bar_duration': each.pattern_AB_bar_duration,
                'label': each.label,
                # 'candles': each.candles.to_json(orient='records')
            }
            list_of_pattern_AB.append(ab_pat)
        
        pattern_AB_df = pd.DataFrame(list_of_pattern_AB)
        DataBase().insert_data(pattern_AB_df,'failed_pattern_ab')

    def load_abc_patterns(self, abc_pivotTrios):
            
            for each in abc_pivotTrios:

                pattern_data = {
                    'pattern_A_start_date': each.pattern_A_start_date,
                    'pattern_A_pivot_date': each.pattern_A_pivot_date,
                    'pattern_A_end_date': each.pattern_A_end_date,
                    'pattern_A_high': each.pattern_A_high,
                    'pattern_A_close':each.pattern_A_close,
                    'pattern_A_open': each.pattern_A_open,
                    'pattern_A_low': each.pattern_A_low,

                    'pattern_B_start_date': each.pattern_B_start_date,
                    'pattern_B_pivot_date': each.pattern_B_pivot_date,
                    'pattern_B_end_date':each.pattern_B_end_date,
                    'pattern_B_high': each.pattern_B_high,
                    'pattern_B_close': each.pattern_B_close,
                    'pattern_B_open': each.pattern_B_open,
                    'pattern_B_low': each.pattern_B_low,

                    'pattern_C_start_date': each.pattern_C_start_date,
                    'pattern_C_pivot_date': each.pattern_C_pivot_date,
                    'pattern_C_end_date': each.pattern_C_end_date,
                    'pattern_C_high': each.pattern_C_high,
                    'pattern_C_close': each.pattern_C_close,
                    'pattern_C_open': each.pattern_C_open,
                    'pattern_C_low': each.pattern_C_low,

                    'pattern_AB_start_date': each.pattern_AB_start_date,
                    'pattern_AB_end_date': each.pattern_AB_end_date,
                    'pattern_AB_bar_length': each.pattern_AB_bar_length,

                    'pattern_ABC_bar_length': each.pattern_ABC_bar_length,
                    'pattern_ABC_start_date': each.pattern_ABC_start_date,
                    'pattern_ABC_end_date': each.pattern_ABC_end_date,
                    # 'pattern_C_bar_retracment': each.pattern_C_bar_retracment,
                    'pattern_C_price_retracement': each.pattern_C_price_retracement,
                    'pattern_BC_bar_length': each.pattern_BC_bar_length,

                    'trade_symbol': each.trade_symbol
                }
        
                pattern_abc.append(pattern_data)
            
            abc_df = pd.DataFrame(pattern_abc)

            DataBase().insert_data(abc_df,'pattern_abc')
    
    def load_abcd_patterns(self, abcd_patterns):
        all = []
        for each in abcd_patterns:
       
            abcd = {
                # 'symbol':stockName,
                'pattern_A_start_date': each.pattern_A_start_date,
                'pattern_A_pivot_date': each.pattern_A_pivot_date,
                'pattern_A_end_date': each.pattern_A_end_date,
                'pattern_A_open': each.pattern_A_open,
                'pattern_A_high': each.pattern_A_high,
                'pattern_A_close': each.pattern_A_close,
                'pattern_A_low': each.pattern_A_low,
                'pattern_B_start_date': each.pattern_B_start_date,
                'pattern_B_pivot_date': each.pattern_B_pivot_date,
                'pattern_B_end_date': each.pattern_B_end_date,
                'pattern_B_open': each.pattern_B_open,
                'pattern_B_high': each.pattern_B_high,
                'pattern_B_close': each.pattern_B_close,
                'pattern_B_low': each.pattern_B_low,
                'pattern_C_start_date': each.pattern_C_start_date,
                'pattern_C_pivot_date': each.pattern_C_pivot_date,
                'pattern_C_end_date': each.pattern_C_end_date,
                'pattern_C_open': each.pattern_C_open,
                'pattern_C_high': each.pattern_C_high,
                'pattern_C_close': each.pattern_C_close,
                'pattern_C_low': each.pattern_C_low,
                'pattern_AB_start_date': each.pattern_AB_start_date,
                'pattern_AB_end_date': each.pattern_AB_end_date,
                'pattern_AB_bar_length': each.pattern_AB_bar_length,
                'pattern_ABC_start_date': each.pattern_ABC_start_date,
                'pattern_ABC_end_date': each.pattern_ABC_end_date,
                'pattern_ABC_bar_length': each.pattern_ABC_bar_length,
                'pattern_C_bar_retracement': each.pattern_C_bar_retracement,
                'pattern_C_price_retracement': each.pattern_C_price_retracement,
                'pattern_BC_bar_length': each.pattern_BC_bar_length,
                # 'pattern_ABCD_id': each.pattern_ABCD_id,
                'pattern_ABCD_bar_length': each.pattern_ABCD_bar_length,
                'pattern_ABCD_start_date': each.pattern_ABCD_start_date,
                'pattern_ABCD_end_date': each.pattern_ABCD_end_date,
                'pattern_D_bar_retracement': each.pattern_D_bar_retracement,
                'pattern_D_price_retracement': each.pattern_D_price_retracement,
                'pattern_CD_bar_length': each.pattern_CD_bar_length,
                'pattern_d_created_date': each.pattern_d_created_date,
                'pivot_A_price': each.pivot_A_price,
                'pivot_B_price': each.pivot_B_price,
                'pivot_C_price': each.pivot_C_price,
                'pivot_D_price': each.pivot_D_price,
                'ab_price_length': each.ab_price_length,
                'bc_price_length': each.bc_price_length,
                'cd_price_length': each.cd_price_length,
                'pivot_D_watchmark': each.pivot_D_watchmark,
                'trade_is_open': each.trade_is_open,
                'trade_is_closed': each.trade_is_closed,
                'trade_entered_date': each.trade_entered_date,
                'trade_entered_price': each.trade_entered_price,
                'trade_exited_date': each.trade_exited_date,
                'trade_exited_price': each.trade_exited_price,
                'trade_risk': each.trade_risk,
                'trade_reward': each.trade_reward,
                'trade_stop_loss': each.trade_stop_loss,
                'trade_duration_bars': each.trade_duration_bars,
                'trade_duration_days': each.trade_duration_days,
                'trade_pnl': each.trade_pnl,
                'trade_result': each.trade_result,
                'trade_return_percentage': each.trade_return_percentage,
                'trade_take_profit': each.trade_take_profit,
                
                'trade_rrr': each.trade_rrr,
                'trade_created': each.trade_created,
                # 'current_date': each.current_date,
                'd_dropped_below_b': each.d_dropped_below_b,
                'symbol':each.trade_symbol,
                'valid': each.valid,
                'x_pivot_price': each.x_pivot_price,
                'x_pivot_date': each.x_pivot_date,
                'd_x_retracement': each.d_x_retracement
            }
            all.append(abcd)
        df = pd.DataFrame(all)
        DataBase().insert_data(df,'pattern_abcd')
    
    

 
    def merge_patterns(self, pattern_abcd):
        """
        Merges patterns from the given list `pattern_abcd` based on their pivot dates.
        The function performs the following operations:
        - Groups patterns by their B and C pivot dates.
        - Finds the highest value of pattern A for each group.
        - Creates a new list of merged patterns with the smallest D date and updates related attributes.

        Args:
            pattern_abcd (list): A list of pattern objects, each containing attributes for B and C pivot dates,
                                and various pattern A and D attributes.

        Returns:
            list: A list of merged pattern objects.
        """

        # Initialize a dictionary to group patterns by their B and C pivot dates
        patterns = {}

        # Populate the dictionary with empty lists for each unique combination of B and C dates
        for each in pattern_abcd:
            b_date = each.pattern_B_pivot_date
            c_date = each.pattern_C_pivot_date
            dates = str(b_date) + '-' + str(c_date)
            if dates not in patterns:
                patterns[dates] = []

        # Assign each pattern object to the appropriate list based on its B and C dates
        for each in pattern_abcd:
            b_date = each.pattern_B_pivot_date
            c_date = each.pattern_C_pivot_date
            dates = str(b_date) + '-' + str(c_date)
            if dates in patterns:
                patterns[dates].append(each)

        # Initialize a new dictionary to store the merged patterns
        new_patterns = {key: [] for key in patterns.keys()}

        # Process each group of patterns to find the highest A value and create the merged patterns
        for key, value in patterns.items():
            d_dropped_below_b_dates = []
            a_high = 0

            # Determine the highest A value for the current group of patterns
            for each in value:
                a = max(each.pattern_A_open, each.pattern_A_close)
                a_high = max(a_high, a)
                d_dropped_below_b_dates.append(each.d_dropped_below_b)

            # Collect all patterns where A matches the highest value found
            for each in value:
                a = max(each.pattern_A_open, each.pattern_A_close)
                if a == a_high:
                    new_patterns[key].append(each)

        # List to store the final merged patterns
        merged_patterns = []

        # Finalize the merging process
        for key, value in new_patterns.items():
            # Find the pattern with the maximum end date
            max_object = max(value, key=lambda obj: obj.pattern_ABCD_end_date)

            # Determine the earliest D date from the patterns
            earliest_date = min(value, key=lambda obj: obj.d_dropped_below_b).d_dropped_below_b

            # Update the earliest D date for the max pattern
            max_object.d_dropped_below_b = earliest_date

            # Add the finalized pattern to the list
            merged_patterns.append(max_object)

            # Update the pattern creation date to the earliest trade entered date
            d_dates = [each.trade_entered_date for each in value]
            for each in value:
                each.pattern_d_created_date = min(d_dates)

        return merged_patterns

    def get_entry_price(self, market, low, high):

        entry_price = None

        match market:

            case 'Bull':

                entry_price = low[0]

            case 'Bear':
            
                entry_price = high[0]


        return entry_price

    def get_risk_and_reward(self, market, abcd, entry_price, rrr):

        match market:

            case 'Bull':

                pivotC_High = StrategyTools().get_high_of_candle(
                    abcd.pattern_C_open, 
                    abcd.pattern_C_close
                )

                reward = pivotC_High - entry_price

                risk = reward / rrr

                return risk, reward
            
            case 'Bear':

                pivotC_Low = StrategyTools().get_low_of_candle(abcd.pivot_C.pivotInfo['open'], abcd.pivot_C.pivotInfo['close'])

                reward = pivotC_Low - entry_price

                risk = reward / rrr

                return risk, reward

    def checkIfQuadIsAlreadyInATrade(self, all_abcds, abcd_id):
        
        
        inTrade = False
        for each in all_abcds:
            if each.tradeInfo['id'] == abcd_id:
                inTrade = True
                break
        return inTrade 
    
    def get_high_of_candle(self, open, close):
                
        high = None

        if close >= open:
            high = close
            
        if open >= close:
            high = open

        return high

    def get_low_of_candle(self, open, close):
        
        low = None

        if close >= open:
            low = open
            
        if close <= open:
            low = close

        return low

    def getBarsInBetweenPivots(self, A_date, date):
        
        i = 0
        while str(A_date) <= str(date(ago=i)):
            i-=1
            
        return i

    def check_bar_length(self, date, start_date):

        try: 
            duration = 0
            while date(ago=-duration) >= start_date:
                duration +=1
            return duration
        
        except Exception as e:
            print(f'check_bar_length {e}')

    def setDateTestingCanStart(self, counter, dateTracker, length, date):
  
        if counter == 0:
            
            
            dateTracker = str(date(length + length))

            counter += 1
            print(counter, dateTracker, length, date)

            return counter, dateTracker

    def preventPullingBarsFromBack(self,setting, currentDate, filterDate):

        match setting:

            case True: 
                if currentDate > filterDate:
                    return True
            case False:
                return True

    def findPivot(self,pivot, pivotObject, date, candle_close: list[dict[float]], candle_open: list[dict[float]], pivot_length: int, pivot_type: str) -> str:
        
        
        openOfPivot = candle_open[-pivot_length]
        closeOfPivot = candle_close[-pivot_length]

        highOfPivot = openOfPivot if openOfPivot > closeOfPivot else closeOfPivot
        lowOfPivot  = openOfPivot if openOfPivot <= closeOfPivot else closeOfPivot


        match pivot_type:

            case 'Bear':

                is_pivot_the_lower = StrategyTools().comparePivotLowToSideCandles(candle_open, candle_close, pivot_length, lowOfPivot)

                return is_pivot_the_lower

            case 'Bull':


                is_pivot_the_higher = StrategyTools().comparePivotHighToSideCandles(candle_open, candle_close, pivot_length, highOfPivot)
    
                return is_pivot_the_higher
    
    def comparePivotLowToSideCandles(self,dataOpen, dataClose, pivotLength, lowOfPivot):

        data_opens_r = []
        data_closes_r = []

        for each in range(0,3):
            data_opens_r.append(dataOpen[-each])
            data_closes_r.append(dataClose[-each])

        x = (pivotLength * 2) + 1

        highest_prices = np.minimum(data_opens_r, data_closes_r)

        pivot_index = len(highest_prices) // 2

        left_list = np.array(highest_prices[2])

        right_list = np.array(highest_prices[0])
        
        if lowOfPivot < left_list and lowOfPivot < right_list:
            return True

    def comparePivotHighToSideCandles(self,dataOpen, dataClose, pivotLength, highOfPivot):

        data_opens_r = []
        data_closes_r = []

        for each in range(0,3):
            data_opens_r.append(dataOpen[-each])
            data_closes_r.append(dataClose[-each])

        highest_prices = np.maximum(data_opens_r, data_closes_r)

        left_list = np.array(highest_prices[2])

        right_list = np.array(highest_prices[0])
        
        if highOfPivot >= left_list and highOfPivot >= right_list:
            return True

    def get_average_size_of_bars_in_pivot(self, pivot_length, data_open, data_close):
        
        candles_in_pivot = pivot_length * 2
        
        # Create an empty list to store the size of each bar in the pivot
        average_of_all_bars = []

        # Loop through the data in reverse order up to the specified length
        for x in range(0, candles_in_pivot):

            # Get the open and close price for the current bar
            open_price = data_open[-x]
            close_price = data_close[-x]

            # Calculate the size of the bar (the absolute difference between the open and close prices)
            candle_size = float("%.2f" % abs(open_price - close_price))

            # Add the size of the bar to the list of all bar sizes
            average_of_all_bars.append(candle_size)

        # Calculate the average size of all bars in the pivot
        average_size = "%.2f" % mean(average_of_all_bars)

        # Return the average size of all bars in the pivot
        return average_size


    def checkPivotSideSteepness(self, setting, close, open, length, percentageChange):

        match setting:

            case True:
                
                leftMidPoint, rightMidPoint = StrategyTools().calculatePercentageDifference(close, open, length)

                if leftMidPoint:
                    if rightMidPoint:
                        

                        LmidPoints = leftMidPoint[::-1]
                        leftSideAverage = float("%.2f"%mean(LmidPoints))

                        RmidPoints = rightMidPoint[::-1]
                        rightSideAverage = float("%.2f"%mean(RmidPoints))
            
                        if leftSideAverage >= percentageChange and rightSideAverage >= percentageChange :
                
                            StrategyTools().calculatePercentageDifference(close, open, length)
        
        
                            return True, leftSideAverage, rightSideAverage

                        else:

                            return False, 0, 0

                
                else:
                    return False, 0, 0

            case False:

                leftMidPoint, rightMidPoint = StrategyTools().getMidPointOfEachCandle(close, open, length)

                if leftMidPoint:
                    if rightMidPoint:

                        LmidPoints = leftMidPoint[::-1]
                        leftSideAverage = float("%.2f"%mean(LmidPoints))

                        RmidPoints = rightMidPoint[::-1]
                        rightSideAverage = float("%.2f"%mean(RmidPoints))
                    
                        return True, leftSideAverage, rightSideAverage
                return True, 0, 0

    def calculatePercentageDifference(self, close, open, length):

        leftMidPoint, rightMidPoint = StrategyTools().getMidPointOfEachCandle(close, open, length)

        leftAverage     = []
        rightAverage    = []

        index = 0
        while index <= (len(leftMidPoint) - 2):

            a = (leftMidPoint[index] - leftMidPoint[index + 1])

            b = (leftMidPoint[index] + leftMidPoint[index + 1]) / 2

            c = (a / b) * 100

            change = c

            leftAverage.append(float("%.2f"%change))
            index +=1
    
        index = 0
        while index <= (len(rightMidPoint) - 2):

            a = (rightMidPoint[index] - rightMidPoint[index + 1])

            b = (rightMidPoint[index] + rightMidPoint[index + 1]) / 2

            c = (a / b) * 100

            change = c

            rightAverage.append(float("%.2f"%change))
            index +=1

        return(leftAverage,rightAverage)
                  
    def getMidPointOfEachCandle(self, close, open, length):

        leftMidPoint    = []
        rightMidPoint   = []

        rightSideOpen, rightSideClose, leftSideOpen, leftSideClose = StrategyTools().getLeftAndRightOpenAndClosesOfPivot(close, open, length)


        for each in range(length):
    
            high = StrategyTools().get_high_of_candle(leftSideOpen[each],leftSideClose[each])
            low  = StrategyTools().get_low_of_candle(leftSideOpen[each],leftSideClose[each])

            leftMidNum = ((float(high) - float(low)) / 2) + float(low)
            leftMidPoint.append(float("%.2f"%leftMidNum))

        for each in range(length):

            high = StrategyTools().get_high_of_candle(rightSideOpen[each],rightSideClose[each])
            low  = StrategyTools().get_low_of_candle(rightSideOpen[each],rightSideClose[each])

            rightMidNum = (float(high) - float(low) / 2) + float(low)
            rightMidPoint.append(float("%.2f"%rightMidNum))

        return leftMidPoint, rightMidPoint

    def getLeftAndRightOpenAndClosesOfPivot(self, close, open, length):
    
        leftSideClose   = []
        leftSideOpen    = []
        rightSideClose  = []
        rightSideOpen   = []
        
        for each in range(length):
            leftSideClose.append(close[-(each + (length+1))])
            leftSideOpen.append(open[-(each + (length+1))])

        for each in range(length):
            rightSideClose.append(close[-each])
            rightSideOpen.append(open[-each])


        return rightSideOpen, rightSideClose, leftSideOpen, leftSideClose

class B:
    
    def create_B(self, b_pivot, date, pivot_length, high, open, close, low, full_length,candle_ids, candle_dates):

        pivotID = len(b_pivot)
        pivotLetter = 'B'
        pivotColor = 'Red'
        paired = False
        pivotDate = date(ago=-pivot_length)
        open = open[-pivot_length]
        high = high[-pivot_length]
        low = low[-pivot_length]
        close = close[-pivot_length]
        startDate = date(ago=-(pivot_length * 2))
        endDate = date(ago=0)
        barsSincePreviousPivot = 0
        daysSincePreviousPivot = 0
        retracementPct = None
        retracementPrice = None,
    

        pivot_B = Pattern_A(
            pivotID,
            pivotLetter,
            pivotColor,
            # paired,
            pivotDate,
            open,
            high,
            low,
            close,
            startDate,
            endDate,
            barsSincePreviousPivot,
            daysSincePreviousPivot,
            retracementPct,
            retracementPrice,
            full_length,
            candle_ids,
            candle_dates
            
        )
        
        return pivot_B

    def add_b_with_group_of_b(self, b_pivots, b_pivot):
                                            
        def is_object_in_list(new_object):
            for obj in b_pivots:
                if obj.pivotInfo['pivotDate'] == new_object.pivotInfo['pivotDate']:
                    return True
            return False

        if not is_object_in_list(b_pivot):

            b_pivots.append(b_pivot)

    def get_days_between_A_and_B(self, date, pivot_length, pivot_A):

        daysBetweenAandB = date(ago=-pivot_length) - pivot_A.pivotInfo['pivotDate']
        daysBetweenAandB = str(daysBetweenAandB).split(' ', 1)[0]

        return daysBetweenAandB

    def check_A_position_to_B_position(self, market, pivot_A, pivot_B):

        b_position = False
                        

        if market == 'Bull':
            if pivot_A.pivotInfo['low'] > pivot_B.pivotInfo['high']:
                b_position =  True

            
        if market == 'Bear':
            if pivot_A.pivotInfo['high'] < pivot_B.pivotInfo['low']:
                b_position =  True


        return b_position

    def check_if_A_is_the_lowest(self,bars_between_A_and_B, candle_high, candle_low, pivot_A_low):

        try:
            is_pivot_A_the_lowest = True

            for each in range(bars_between_A_and_B):

                    if candle_high[-each] < pivot_A_low:
                
                        is_pivot_A_the_lowest = False
                    
                    if candle_low[-each] < pivot_A_low:
                        is_pivot_A_the_lowest = False
                
            return is_pivot_A_the_lowest

        except Exception as e:
            print(f'check_if_A_is_the_lowest {e}')
                                
    def check_if_A_is_the_highest(
        self,
        bars_between_A_and_B, 
        candle_high, 
        candle_low, 
        pivot_A_high,

        ):

        try:    
            isPivotA_Highest = True
            
            for each in range(bars_between_A_and_B):

                if candle_high[-each] > pivot_A_high:
                    isPivotA_Highest = False  
                    
                if candle_low[-each] > pivot_A_high:
                    isPivotA_Highest = False        
            
            return isPivotA_Highest
        except Exception as e:
            print(f'check_if_A_is_the_highest {e}')

    def check_if_B_is_the_lowest(self,bars_between_A_and_B, candle_open, candle_close, pattern_B_low):

        try:     
            isPivotB_Lowest = True

            for each in range(bars_between_A_and_B):

                if candle_open[-each] < pattern_B_low:
                    isPivotB_Lowest =  False
                
                if candle_close[-each] < pattern_B_low: 
                    isPivotB_Lowest = False
            return isPivotB_Lowest

        except Exception as e:
            print(f'check_if_B_is_the_lowest {e}')

    def check_if_B_is_the_highest(self,bars_between_A_and_B, candle_high, candle_low, pattern_B_high):

        try:

            isPivotB_Highest = True
            for each in range(bars_between_A_and_B):
                if candle_high[-each] > pattern_B_high:
                    isPivotB_Highest = False

                if candle_low[-each] > pattern_B_high: 
                    isPivotB_Highest = False
        
            return isPivotB_Highest
        except Exception as e:
            print(f'check_if_B_is_the_highest {e}')

    def find_B(self,pivot, pivotOjbect, date, market, close, open, pivot_length):

        pivotType = None

        match market:

            case 'Bull':
                
                pivotType = StrategyTools().findPivot(pivot, pivotOjbect, date, close, open, pivot_length, 'Bear')

            case 'Bear':

                pivotType = StrategyTools().findPivot(date, close, open, pivot_length, 'Bull')

        return pivotType

    # def check_if_A_to_B_is_correct_shape(self,market, is_A_the_highest, is_B_the_lowest, is_A_the_lowest,is_B_the_highest):

    #     match market:

    #         case 'Bull':
                
    #             if is_A_the_highest and is_B_the_lowest:

    #                 return True
                
    #             elif not is_A_the_highest  or not is_B_the_lowest:
                    
    #                 return False

    #         case 'Bear':

    #             if is_A_the_lowest and is_B_the_highest:

    #                 return True
                
    #             elif not is_A_the_lowest  or not is_B_the_highest:

    #                 return False
    
    # def check_ab_is_start_of_abcd(self,market, pivot_A_high, pivot_B_low):


    #     try:

    #         match market:

    #             case 'Bull':
                    
    #                 return pivot_A_high > pivot_B_low
                
    #             case 'Bear':

    #                 return pivot_A_high < pivot_B_low
    #     except Exception as e:
    #         print(f'check_ab_is_start_of_abcd {e}')

class C:

    def create_pivot_c(self,c_pivots, b_to_c_bars, date, pivot_length, high, open, close, low, pivot_pair, c_length, candle_ids, candle_dates):


        # Prepare data.
        pivotID = c_pivots
        pivotLetter = 'C'
        pivotColor = 'Red'
        # paired = False
        pivotDate = date(ago=-pivot_length)
        open = open[-int(pivot_length)]
        high = high[-int(pivot_length)]
        low = low[-int(pivot_length)]
        close = close[-int(pivot_length)]
        startDate = date(ago=-(int(pivot_length) * 2))
        endDate = date(ago=0)
        barsSincePreviousPivot = b_to_c_bars

        daysSincePreviousPivot = pivotDate - pivot_pair.pivot_B.pivotInfo['pivotDate']
        retracementPct = None
        retracementPrice = None

        # Create pivot.
        pivot_C = Pattern_A(
            pivotID,
            pivotLetter,
            pivotColor,
            # paired,
            pivotDate,
            open,
            high,
            low,
            close,
            startDate,
            endDate,
            barsSincePreviousPivot,
            daysSincePreviousPivot,
            retracementPct,
            retracementPrice,
            c_length,
            candle_ids,
            candle_dates
            
        )

        return pivot_C

    def is_b_low_and_c_high(
            self,
            pivot_C_open, 
            pivot_C_close, 
            pivot_B_open,
            pivot_B_close,
            b_to_c_bars, 
            data_open, 
            data_close):
        
        try:
            pivotC_HighEnd = StrategyTools().get_high_of_candle(pivot_C_open, pivot_C_close)
            pivot_b_bottom = StrategyTools().get_low_of_candle(pivot_B_open ,pivot_B_close)


            isPivotC_Highest = True
            is_pivot_b_the_lowest = True

            try: 

                for each in range(int(b_to_c_bars)):

                    currentCandleOpen = data_open[-each]
                    currentCandleClose = data_close[-each]

                    isOpenAboveThenPivotCHigh = currentCandleOpen > pivotC_HighEnd
                    isCloseAboveThenPivotCHigh = currentCandleClose > pivotC_HighEnd

                    is_open_below_pivot_b_bottom = currentCandleOpen < pivot_b_bottom
                    is_close_below_pivot_b_bottom = currentCandleClose < pivot_b_bottom

                    if isOpenAboveThenPivotCHigh or isCloseAboveThenPivotCHigh:
                        isPivotC_Highest = False
                    
                    if is_open_below_pivot_b_bottom or is_close_below_pivot_b_bottom:
                        is_pivot_b_the_lowest = False
            except Exception as e:
                print(f'for each in range(b_to_c_bars): {e}')

            return isPivotC_Highest, is_pivot_b_the_lowest



        except Exception as e:
            print(f'get_high_of_candle get_low_of_candle {e}')
        
    def is_b_high_and_c_low(
            self,
            pivot_C_open, 
            pivot_C_close, 
            pivot_B_open,
            pivot_B_close,
            b_to_c_bars, 
            data_open, 
            data_close):
        pivot_c_low = StrategyTools().get_low_of_candle(pivot_C_open, pivot_C_close)
        pivot_b_high = StrategyTools().get_high_of_candle(pivot_B_open ,pivot_B_close)

        is_pivot_c_lowest = True
        is_pivot_b_highest = True

        for each in range(b_to_c_bars):

            currentCandleOpen = data_open[-each]
            currentCandleClose = data_close[-each]

            is_candle_open_lower_then_c_low = currentCandleOpen < pivot_c_low
            is_candle_close_lower_then_c_low = currentCandleClose < pivot_c_low

            is_candle_open_higher_then_b_high = currentCandleOpen > pivot_b_high
            is_candle_close_higher_then_b_high = currentCandleClose > pivot_b_high

            if is_candle_open_lower_then_c_low or is_candle_close_lower_then_c_low:
                is_pivot_c_lowest = False
            
            if is_candle_open_higher_then_b_high or is_candle_close_higher_then_b_high:
                is_pivot_b_highest = False

        return is_pivot_c_lowest, is_pivot_b_highest

    def get_bull_retracement(
            self,
            pattern_C_high,
            pattern_B_low,
            pattern_A_high):

        c_High = float(pattern_C_high)

        b_Low = float(pattern_B_low)

        a_High = float(pattern_A_high)
        
        b_c_retracement = ((c_High - b_Low) / (a_High - b_Low)) * 100

        return b_c_retracement

    def get_bear_retracement(
            self,
            pattern_C_low,
            pattern_B_high,
            pattern_A_low
    ):

        c_low = float(pattern_C_low)
        b_high = float(pattern_B_high)
        a_low = float(pattern_A_low)
        b_c_retracement = '%.2f'%((b_high - c_low) / (b_high - a_low) * 100)

        return b_c_retracement

    def check_bull_shape(self,pivot_C_low, pivot_C_high, pivot_pair):

        c_low_above_b_high = pivot_C_low > pivot_pair.pattern_B_high

        c_high_below_a_low = pivot_C_high < pivot_pair.pattern_A_low

        return c_low_above_b_high, c_high_below_a_low

    def check_bear_shape(self,pivot_C_low, pivot_C_high, pivot_pair):
        
        c_low = pivot_C_low
        c_high = pivot_C_high

        b_low = pivot_pair.pattern_B_low
        a_high = pivot_pair.pattern_A_high

        c_high_below_b_low = c_high < b_low
        c_low_above_a_high = c_low > a_high

        return c_high_below_b_low, c_low_above_a_high

    def check_if_c_is_the_high_and_if_b_is_the_low(
            self,
            settings,
            pattern_C_pivot_open,
            pattern_C_pivot_close,
            pattern_B_pivot_open,
            pattern_B_pivot_close,
            bars_passed,
            data_open,
            data_close
            ):
        c_position = None
        b_position = None


        try:
        
            if settings['market'] == 'Bull':
                c_position, b_position = C().is_b_low_and_c_high(
                    pattern_C_pivot_open,
                    pattern_C_pivot_close,
                    pattern_B_pivot_open,
                    pattern_B_pivot_close,
                    bars_passed, 
                    data_open, 
                    data_close,
                    )
                
        except Exception as e:
            print(f'check_if_c_is_the_high_and_if_b_is_the_low {e}')
        

        if settings['market'] == 'Bear':
            c_position, b_position = C().is_b_high_and_c_low(
                pattern_C_pivot_open,
                pattern_C_pivot_close,
                pattern_B_pivot_open,
                pattern_B_pivot_close,
                bars_passed, 
                data_open, 
                data_close,)
            
        if c_position and b_position: 
            return True
        else:
            return False
                    
    def get_pattern_c_price_retracement(
            self,
        settings,
        pattern_C_high,
        pattern_C_low,
        pattern_B_high,
        pattern_B_low,
        pattern_A_high,
        pattern_A_low,

    ):
        try:
            c_retracement_price_retracement_ = None

            if settings['market'] == 'Bull':
                c_retracement_price_retracement_ = C().get_bull_retracement(
                    pattern_C_high,
                    pattern_B_low,
                    pattern_A_high)

            elif settings['market'] == 'Bear':
                c_retracement_price_retracement_ = C().get_bear_retracement(
                    pattern_C_low,
                    pattern_B_high,
                    pattern_A_low)

            return c_retracement_price_retracement_
        except Exception as e:
            print(e)

    def check_pattern_c_price_retracement(
            self,
            settings,
            data_high,
            data_low,
            pivot_pair,

    ):
        
        c_retracement_price_retracement_ = None

        if settings['market'] == 'Bull':
            c_retracement_price_retracement_ = C().get_bull_retracement(
            data_high[-settings['pivotLength']], 
            pivot_pair)

        elif settings['market'] == 'Bear':
            c_retracement_price_retracement_ = C().get_bear_retracement(
            data_low[-settings['pivotLength']],
            pivot_pair)

        if float(c_retracement_price_retracement_) >= 1 and float(c_retracement_price_retracement_) <= 100:
            return True
        else:
            return False
        
    def check_pattern_ABC_shape(
            self,
        settings,
        data_high,
        data_low,
        pivot_pair,

        
        ):
        c_shape = None
        b_shape = None

        if  settings['market'] == 'Bear':

            c_shape, b_shape = C().check_bear_shape(
                data_low[-settings['pivotLength']],
                data_high[-settings['pivotLength']], 
                pivot_pair)

        if settings['market'] == 'Bull':

            c_shape, b_shape = C().check_bull_shape(
                data_low[-settings['pivotLength']],
                data_high[-settings['pivotLength']], 
                pivot_pair)

        if c_shape and b_shape:

            return True
        else:

            return False

class D:
        
    def get_bull_d_to_b_retracement(self,pivot_trio, d_low):

        c_High = float(pivot_trio.pivot_C.pivotInfo['high'])

        d_Low = float(d_low)

        b_Low = float(pivot_trio.pivot_B.pivotInfo['low'])

        cd_retracement = '%.2f'%(((c_High - d_Low) / (c_High - b_Low)) * 100)

        return cd_retracement

    def get_bear_d_to_b_retracement(self,pivot_trio, d_high):

        c_low = float(pivot_trio.pivot_C.pivotInfo['low'])

        d_high = float(d_high)

        b_high = float(pivot_trio.pivot_B.pivotInfo['high'])

        bear_cd_retracement = '%.2f'%((d_high - c_low) / (b_high - c_low) * 100)

        return bear_cd_retracement

    def check_bull_c_position(
            self,
        c_to_d_bar_length, 
        open, 
        close, 
        pivot_trio):

        try: 
            isPivotC_Hightest = True

            for each in range(c_to_d_bar_length):

                open_ = open[-each]

                close_ = close[-each]

                pivotC_open = pivot_trio.pattern_C_open

                pivotC_close = pivot_trio.pattern_C_close

                pivotC_High = StrategyTools().get_high_of_candle(pivotC_open, pivotC_close)

                candle_High = StrategyTools().get_high_of_candle(open_, close_)

                candleClosesAboveC = pivotC_High < candle_High

                if candleClosesAboveC:

                    isPivotC_Hightest = False

            return isPivotC_Hightest
        except Exception as e:

            print(f'check bull c position {e}')

    def check_bear_c_position(
            self,
        c_to_d_bar_length, 
        open, 
        close, 
        pivot_trio):

        try: 
            is_c_lowest = True

            for each in range(c_to_d_bar_length):

                open_ = open[-each]

                close_ = close[-each]

                pivotC_open = pivot_trio.pattern_C_open

                pivotC_close = pivot_trio.pattern_C_close

                c_low = StrategyTools().get_low_of_candle(pivotC_open, pivotC_close)

                current_candle_low = StrategyTools().get_low_of_candle(open_, close_)
                
                candle_is_lower_then_c = current_candle_low < c_low

                if candle_is_lower_then_c:

                    is_c_lowest = False

            return is_c_lowest
        except Exception as e:

            print(f'check bull c position {e}')

    def check_bull_d_position(self,c_to_d_bar_length, open, close):

        isPivotD_Lowest = True

        for each in range(c_to_d_bar_length):

            open_ = open[-each]

            close_ = close[-each]

            pivotD_open = open[-1]

            pivotD_close = close[-1]

            pivotD_Low = StrategyTools().get_low_of_candle(pivotD_open, pivotD_close)

            candle_Low = StrategyTools().get_low_of_candle(open_, close_)

            candleClosesBelowD = pivotD_Low > candle_Low

            if candleClosesBelowD:

                isPivotD_Lowest = False
                
        return isPivotD_Lowest

    def check_bear_d_position(self,c_to_d_bar_length, open, close):

        is_d_the_highest = True

        for each in range(c_to_d_bar_length):

            open_ = open[-each]

            close_ = close[-each]

            pivotD_open = open[-1]

            pivotD_close = close[-1]

            d_high = StrategyTools().get_high_of_candle(pivotD_open, pivotD_close)

            current_candle_high = StrategyTools().get_high_of_candle(open_, close_)

            candle_is_higher_then_d = current_candle_high > d_high

            if candle_is_higher_then_d:

                is_d_the_highest = False

        return is_d_the_highest
                    
    def create_d(self,d_pivots, date, high, open, close, low, c_to_d_bar_length, pivot_trio, d_length, candle_ids,candle_dates):

        # Prepare pivot D data.
        pivotID = d_pivots
        pivotLetter = 'D'
        pivotColor = 'Red'
        paired = False
        pivotDate =  date(ago=0)
        open = open[0]
        high = high[0]
        low = low[0]
        close = close[0]
        startDate = date(ago=0)
        endDate = date(ago=0)
        barsSincePreviousPivot = c_to_d_bar_length
        daysSincePreviousPivot = pivotDate - pivot_trio.pivot_C.pivotInfo['pivotDate']
        retracementPct = None
        retracementPrice = None

        # Create pivot D.
        pivot_D = Pattern_A(
            pivotID,
            pivotLetter,
            pivotColor,
            # paired,
            pivotDate,
            open,
            high,
            low,
            close,
            startDate,
            endDate,
            barsSincePreviousPivot,
            daysSincePreviousPivot,
            retracementPct,
            retracementPrice,
            3,
            candle_ids,
            candle_dates
            
        )

        return pivot_D

    def get_bull_d_to_a_retracement(self,pattern_C_high, pattern_A_high, pattern_B_low, pattern_D_low):


        cd_retracement = '%.2f'%((pattern_C_high - pattern_D_low) / (pattern_A_high - pattern_B_low) * 100)

        return cd_retracement

    def get_bear_d_to_a_retracement(self,pattern_C_low, pattern_A_low, pattern_B_high, d_high):

        # Get c to d price length pct.
        c_low = float(pattern_C_low)
        d_high = float(d_high)
        a_low = float(pattern_A_low)
        b_high = float(pattern_B_high)

        cd_retracement = '%.2f'%((d_high - c_low) / (b_high - a_low) * 100)

        return cd_retracement

    def get_d_to_b_retracement(self,market, pivot_trio, low, high):

        d_to_b_retracement = None

        if market == 'Bull':
            d_to_b_retracement = D().get_bull_d_to_b_retracement(pivot_trio, low)

        elif market == 'Bear':
            d_to_b_retracement = D().get_bear_d_to_b_retracement(pivot_trio, high)


        return d_to_b_retracement

    def get_d_to_a_retracement(self,market, pattern_ABC,low):

        try:
        
            candle_C_top = pattern_ABC.pattern_C_open if pattern_ABC.pattern_C_open < pattern_ABC.pattern_C_close else pattern_ABC.pattern_C_close
            candle_A_top = pattern_ABC.pattern_A_open if pattern_ABC.pattern_A_open < pattern_ABC.pattern_A_close else pattern_ABC.pattern_A_close
            candle_B_bot = pattern_ABC.pattern_B_open if pattern_ABC.pattern_B_open > pattern_ABC.pattern_B_close else pattern_ABC.pattern_B_close
            
            if market == 'Bull':

                if candle_A_top == candle_B_bot:
                    cd_retracement = None 
                    return cd_retracement
                else:
                    cd_retracement = (candle_C_top - low) / (candle_A_top - candle_B_bot) * 100
                    return cd_retracement

            elif market == 'Bear':
                return

        
        except Exception as e:
            # print(market)
            # print(pattern_ABC)
            # print(low)
            # print(candle_A_top)
            # print(candle_C_top)
            # print(candle_B_bot)
            print(e)
            sys.exit()

    def get_c_position(self,market, c_to_d_bar_length, pivotTrio, open, close):

        c_position = None
        match market:

            case 'Bull':
                c_position = D().check_bull_c_position(c_to_d_bar_length, open, close, pivotTrio)

            case 'Bear':
                c_position = D().check_bear_c_position(c_to_d_bar_length, open, close, pivotTrio)

        return c_position

    def get_d_position(self,market, c_to_d_bar_length, pivotTrio, open, close):

        c_position = None

        match market:

            case 'Bull':
                c_position = D().check_bull_d_position(c_to_d_bar_length, open, close)

            case 'Bear':
                c_position = D().check_bear_d_position(c_to_d_bar_length, open, close)

        return c_position

class Core:

    def check_for_pivot_A(
        self,
        date,
        data_close,
        data_open,
        pivot_length,
        market,
        a_pivots,
        data_high,
        data_low,
        symbol,
        data_volume
        ):
                
        try:
            
            candle_one_high = data_high[0]
            pivot_high = data_high[-1]
            candle_three_high = data_high[-2]

            if pivot_high >= candle_one_high and pivot_high >= candle_three_high:
                candles = []
                for index in [0,-1,-2]:
                    
                    candles.append({
                    'date': date(ago=index),
                    'open': data_open[index],
                    'high': data_high[index],
                    'low':data_low[index],
                    'close': data_close[index],
                    'volume': data_volume[index]
                    })

                df = pd.DataFrame(candles)
            
                pattern_A = Pattern_A(
                    date(ago=-2), # Pattern A start date
                    date(ago=-1), # Pattern A pivot date
                    date(ago=0), # Pattern A end date
                    data_high[-pivot_length],  # Pattern A pivot high
                    data_close[-pivot_length], # Pattern A pivot close
                    data_open[-pivot_length], # Pattern A pivot open
                    data_low[-pivot_length], # Pattern A pivot low
                    symbol,
                    df
                )


                a_pivots.append(pattern_A)
                        
        except Exception as e:
            print(f'check_for_pivot_A {e}')

    def check_for_pivot_B(self, a_pivots, date, market, data_close, data_open, pivot_length,
                      data_low, data_high, matched_pivot_ones, ab_pivotPairs,
                      failed_AB_patterns, symbol, data_volume):
        
        def create_pattern_AB(label: str) -> Pattern_AB:
            return Pattern_AB(
                pivot_A.pattern_A_start_date,
                pivot_A.pattern_A_pivot_date,
                pivot_A.pattern_A_end_date,
                pivot_A.pattern_A_high,
                pivot_A.pattern_A_close,
                pivot_A.pattern_A_open,
                pivot_A.pattern_A_low,
                date(ago=-(pivot_length * 2)),  # Pattern B start date
                date(ago=-pivot_length),       # Pattern B pivot date
                date(ago=0),                   # Pattern B end date
                data_high[-pivot_length],      # Pattern B pivot high
                data_close[-pivot_length],     # Pattern B pivot close
                data_open[-pivot_length],      # Pattern B pivot open
                data_low[-pivot_length],       # Pattern B pivot low
                symbol,
                pivot_A.pattern_AB_bar_duration,
                label, 
                pivot_A.candles
                                )
        
        try:
            to_remove = []
       
            for pivot_A in reversed(a_pivots):
                
                pivot_A.pattern_AB_bar_duration += 1
                
                # REMOVE PATTERNS WHERE CURRENT CANDLE IS HIGHER THAN A
                if data_high[0] > pivot_A.pattern_A_high:
                    to_remove.append(pivot_A)
            
                # ADD CURRENT CANDLE BAR DF TO ALL CANDLES DF
                current_bar = [{
                    'date': date(ago=0),
                    'open': data_open[0],
                    'high': data_high[0],
                    'low':data_low[0],
                    'close': data_close[0],
                    'volume': data_volume[0]
                    }]
                df_new = pd.DataFrame(current_bar)
                if not (pivot_A.candles['date'] == date(ago=0)).any():
                    pivot_A.candles = pd.concat([pivot_A.candles, df_new], ignore_index=True)
                    pivot_A.candles.sort_values(by='date', inplace=True)
                    pivot_A.candles.reset_index(drop=True, inplace=True)
                      
                    # Check for Pivot-B ---
                    candle_one = data_low[0]
                    candle_two = data_low[-1]
                    candle_three = data_low[-2]
                    if candle_two <= candle_one and candle_two <= candle_three:

                        # Check if A is the High ---
                        if pivot_A.pattern_A_high >= pivot_A.candles['high'].max():
                        
                            # a = pd.to_datetime('2020-01-15').date()
                            # b = pd.to_datetime('2020-03-05').date()

                       
                            # if pivot_A.pattern_A_pivot_date == a:
                            #     if date(ago=0) == b:
                            #         print( pivot_A.candles)
                            #         print(candle_two, pivot_A.candles['low'].min())
                            #         sys.exit()
                        #     # Checn if B is the Low ---
                            if candle_two <= pivot_A.candles['low'].min():

                            
                                pattern_AB = create_pattern_AB("Success")
                                matched_pivot_ones.append(pivot_A.pattern_A_pivot_date)
                                ab_pivotPairs.append(pattern_AB)
                                
                          
                    #         else:
                    #             failed_AB_patterns.append(create_pattern_AB("Pivot B is not the low"))
                    #     else:
                    #         failed_AB_patterns.append(create_pattern_AB("Pivot A is not the high"))
                    # else:
                    #     failed_AB_patterns.append(create_pattern_AB("No Pivot B Found"))

            for pivot_A in to_remove:
                a_pivots.remove(pivot_A)

        except Exception as e:
            print(f'check_for_pivot_B {e}')

    def check_for_pivot_C(self,ab_pivotPairs,date, data_close,data_open,pivot_length,settings,
                    data_high,data_low,market,abc_pivot_trios,symbol, data_volume):

        to_remove = []

        # Check each AB pattern for a ABC
        for pivotPair in list((ab_pivotPairs)):

            # Add to current pattern duration
            pivotPair.pattern_BC_bar_duration += 1
            
            # Track patterns where..
            # 1. current candle high is higher than high of A
            # 2. current candle low is lower than low of B
            if data_high[0] > pivotPair.pattern_A_high or data_low[0] < pivotPair.pattern_B_low:
                to_remove.append(pivotPair)

            # Add current bar to df data
            current_bar = [{
                'date': date(ago=0),
                'open': data_open[0],
                'high': data_high[0],
                'low':data_low[0],
                'close': data_close[0],
                'volume': data_volume[0]
                }]
            df_new = pd.DataFrame(current_bar)
            if not (pivotPair.candles['date'] == date(ago=0)).any():
                pivotPair.candles = pd.concat([pivotPair.candles, df_new], ignore_index=True)
                pivotPair.candles.sort_values(by='date', inplace=True)
                pivotPair.candles.reset_index(drop=True, inplace=True)

            # Check for a pivot
            left_candle = data_high[0]
            pivot_candle = data_high[-1]
            right_candle = data_high[-2]
            if pivot_candle >= left_candle and pivot_candle >= right_candle:

                # Check if C-High is higher than B-Low
                if pivot_candle >= pivotPair.pattern_B_low:

                    # Check if C-High is the high from point B
                    pivot_b = pivotPair.pattern_B_pivot_date
                   
                    df = pivotPair.candles.copy()
                    df['date'] = pd.to_datetime(df['date'])

                    # Convert your pivot_b and date(ago=-0) to Timestamps
                    pivot_b_ts = pd.Timestamp(pivot_b)
                    my_date_ts = pd.Timestamp(date(ago=-3))

                    # Filter using bitwise & and parentheses
                    df = df[(df['date'] >= pivot_b_ts) & (df['date'] <= my_date_ts)]

                    if pivot_candle > df['high'].max():
                   
                        if pivotPair.pattern_B_low <= df['low'].min():

                            a_high = pivotPair.pattern_A_high
                            b_low = pivotPair.pattern_B_low
                            c_high = data_high[-1]

                            ab = a_high - b_low
                            bc = c_high - b_low

                            bc_retracement = (bc / ab) * 100

                            # if bc_retracement > 38 and bc_retracement < 62:
                       
                            pattern_ABC = Pattern_ABC(

                                # Pattern A
                                pivotPair.pattern_A_start_date,
                                pivotPair.pattern_A_pivot_date, 
                                pivotPair.pattern_A_end_date,
                                pivotPair.pattern_A_high,
                                pivotPair.pattern_A_close,
                                pivotPair.pattern_A_open,
                                pivotPair.pattern_A_low,

                                # Pattern B
                                pivotPair.pattern_B_start_date,
                                pivotPair.pattern_B_pivot_date, 
                                pivotPair.pattern_B_end_date,
                                pivotPair.pattern_B_high,
                                pivotPair.pattern_B_close,
                                pivotPair.pattern_B_open,
                                pivotPair.pattern_B_low,

                                date(ago=-(pivot_length * 2)),   # Pattern C start date
                                date(ago=-pivot_length),   # Pattern C start dat
                                date(ago=0),   # Pattern C start date
                                data_high[-pivot_length],   # Pattern C start date        
                                data_close[-pivot_length],   # Pattern C start date
                                data_open[-pivot_length],  # Pattern C start date      
                                data_low[-pivot_length],  # Pattern C start date

                                # PATTERN AB
                                pivotPair.pattern_AB_start_date,
                                pivotPair.pattern_AB_end_date,
                                pivotPair.pattern_AB_bar_duration,

                                # PATTERN ABC
                                pivotPair.pattern_BC_bar_duration + pivotPair.pattern_AB_bar_duration,
                                pivotPair.pattern_A_start_date,
                                date(0),
                                0,
                                bc_retracement,
                                pivotPair.pattern_BC_bar_duration,
                                symbol,

                            )                               
                            abc_pivot_trios.append(pattern_ABC)

        for pivotPair in to_remove:
        
            try:
                ab_pivotPairs.remove(pivotPair)
            except Exception as e:
                print(f"Remove error: {e}")   
                sys.exit()

    def check_for_pivot_D(
        self,
        abc_pivotTrios, 
        market,
        data_low,
        data_high,
        date,
        data_open,
        data_close,
        pattern_abcd,
        symbol
        ):

        try:

            patterns_to_remove = []
            for pivotTrioID, pattern_ABC in enumerate(reversed(abc_pivotTrios)):
                    
                # ==== Remove pattern if current candle is higher than C-high
                pivot_C_top = pattern_ABC.pattern_C_high
                current_candle_top = data_high[-1]
                if current_candle_top > pivot_C_top:
                    patterns_to_remove.append(pattern_ABC)
                
                # Get price lengths
                candle_A_top = pattern_ABC.pattern_A_high
                candle_B_bot = pattern_ABC.pattern_B_low 
                candle_C_top = pattern_ABC.pattern_C_high
                candle_D_bot = data_open[0] if data_open[0] < data_close[0] else data_close[0]
                ab_length = candle_A_top - candle_B_bot
                bc_length = candle_C_top - candle_B_bot
                cd_length = candle_C_top - candle_D_bot

                c_to_d_bar_length = StrategyTools().check_bar_length(date, pattern_ABC.pattern_C_pivot_date) - 1
                abcd_bar_length = StrategyTools().check_bar_length(date, pattern_ABC.pattern_A_start_date)

                # C high and D low
                # c_position = D().get_c_position(market, c_to_d_bar_length, pattern_ABC, pattern_ABC.pattern_C_high, pattern_ABC.pattern_C)
                d_position = D().get_d_position(market, c_to_d_bar_length, pattern_ABC, data_high, data_low)
                
                # Check for a pivot
                right_candle = data_low[0]
                pivot_candle = data_low[-1]
                left_candle = data_low[-2]
                if pivot_candle <= left_candle and pivot_candle <= right_candle:
                    # if c_position:
                        if d_position:
                            if data_low[-1] < pattern_ABC.pattern_B_low:

                                a_high = pattern_ABC.pattern_A_high
                                b_low = pattern_ABC.pattern_B_low
                                c_high = pattern_ABC.pattern_C_high
                                d_low = pivot_candle

                                ab = a_high - b_low
                                bc = c_high - b_low
                                cd = c_high - d_low
                                cd_retracement = round((cd / ab) * 100, 2)

                                # Bar retracement
                          
                                cd_pct_of_ab = round(((c_to_d_bar_length)/ pattern_ABC.pattern_AB_bar_length) * 100,2)


                                abcd = Pattern_ABCD(
                                    'x_pivot_price',
                                    'x_pivot_date',
                                    'd_x_retracement', 
                                    # PATTERN A
                                    pattern_ABC.pattern_A_start_date,
                                    pattern_ABC.pattern_A_pivot_date, 
                                    pattern_ABC.pattern_A_end_date,
                                    pattern_ABC.pattern_A_high,
                                    pattern_ABC.pattern_A_close,
                                    pattern_ABC.pattern_A_open,
                                    pattern_ABC.pattern_A_low,
                                    # Pattern B
                                    pattern_ABC.pattern_B_start_date,
                                    pattern_ABC.pattern_B_pivot_date, 
                                    pattern_ABC.pattern_B_end_date,
                                    pattern_ABC.pattern_B_high,
                                    pattern_ABC.pattern_B_close,
                                    pattern_ABC.pattern_B_open,
                                    pattern_ABC.pattern_B_low,
                                    # Pattern C
                                    pattern_ABC.pattern_C_start_date,
                                    pattern_ABC.pattern_C_pivot_date, 
                                    pattern_ABC.pattern_C_end_date,
                                    pattern_ABC.pattern_C_high,
                                    pattern_ABC.pattern_C_close,
                                    pattern_ABC.pattern_C_open,
                                    pattern_ABC.pattern_C_low,
                                    # PATTERN AB
                                    pattern_ABC.pattern_AB_start_date,
                                    pattern_ABC.pattern_AB_end_date,
                                    pattern_ABC.pattern_AB_bar_length,
                                    # PATTERN ABC
                                    pattern_ABC.pattern_ABC_bar_length,
                                    pattern_ABC.pattern_ABC_start_date,
                                    pattern_ABC.pattern_ABC_end_date,
                                    pattern_ABC.pattern_C_bar_retracement,
                                    pattern_ABC.pattern_C_price_retracement,
                                    pattern_ABC.pattern_BC_bar_length,
                                    # PATTERN ABCD
                                    uuid.uuid4(),
                                    abcd_bar_length,
                                    pattern_ABC.pattern_A_start_date,
                                    date(ago=0),
                                    cd_pct_of_ab,
                                    cd_retracement, 
                                    c_to_d_bar_length,
                                    symbol,
                                    # PIVOT PRICES
                                    candle_A_top,
                                    candle_B_bot,
                                    candle_C_top,
                                    candle_D_bot,
                                    # PATTERN PRICE LENGTHS
                                    ab_length,
                                    bc_length,
                                    cd_length,
                                    'pivot_D_watchmark'
                                )
                                pattern_ABC.pattern_ABC_found_D = True
                                pattern_abcd.append(abcd)


            for abc_pattern in patterns_to_remove:
        
                try:
                    abc_pivotTrios.remove(abc_pattern)
                except Exception as e:
                    print(f"Remove error: {e}")   
                    sys.exit()

            

        except Exception as e:
            print(f'check_for_pivot_D {e}')

    def enter_trade(self,pattern_abcd, date, volume, market,data_low, data_high, rrr):
        
        try:
                
            for abcdID, abcd in enumerate(pattern_abcd):
            

                if abcd.trade_created == False:
                    abcd.trade_created = True
                    abcd.trade_is_open = True
                    # Enter Trade
                    abcd.trade_is_open = True
                    abcd.trade_is_closed = False
                    abcd.trade_entered_date = date(ago=-1)
                    abcd.trade_entered_price = data_low[-1]
                    # abcd.risk, abcd.reward = get_risk_and_reward(market, abcd, abcd.trade_entered_price, abcd.rrr)
                    abcd.trade_reward = float(abcd.pivot_C_price) - float(abcd.pivot_D_price) 
                    abcd.trade_risk = float(abcd.pivot_C_price) - float(abcd.pivot_D_price) 
                    abcd.trade_take_profit = float(abcd.trade_entered_price + (float(abcd.pattern_C_high) - float(data_low[-1])))
                    abcd.trade_stop_loss = float(abcd.trade_entered_price - (float(abcd.pattern_C_high) - float(data_low[-1])))
                    abcd.d_dropped_below_b = date(ago=-1)
                    
                
        
        except Exception as e:
            print(f'enter trade {e}')
                
    def exit_trade(self,pattern_abcd, data_open, data_close, date, data, ids, trade_symbol):

        try:
            for each in pattern_abcd:
                

                if each.trade_is_open == True and each.trade_is_closed == False:
                    
                    # TRACK TRADE DURATION
                    each.trade_duration_bars += 1
                    each.current_date = date(ago=0)

                    # TRACK IF CURRENT PRICE IS HIGHER OR LOWER THAN ENTRY PRICE
                    z = each.trade_entered_price - data_close[0]
                    y = data_close[0] - each.trade_entered_price

                    # ASSIGN DIFFERENCE TO PNL
                    each.trade_pnl = round(y if data_close[0] > each.trade_entered_price else z,2)
                    
                    # TRACK LOWEST PRICE DROPPED
                    # low = get_low_of_candle(data_open[0], data_close[0])
                    
                    # if low < float(each.tradeInfo['lowest_price_dropped']):
                    #     each.tradeInfo['lowest_price_dropped'] = str(low)


                    is_open_higher_than_take_profit = data_open[0] > each.trade_take_profit
                    is_close_higher_then_take_profit = data_close[0] > each.trade_take_profit

                    is_open_lower_than_stop_loss = data_open[0] < each.trade_stop_loss
                    is_close_lower_than_stop_loss = data_close[0] < each.trade_stop_loss

                    take_profit_market_hit = is_open_higher_than_take_profit or is_close_higher_then_take_profit
                    stop_loss_market_hit = is_open_lower_than_stop_loss or is_close_lower_than_stop_loss

                    each.trade_candle_ids = []

                    # GATHER CANDLE IDS OF ABCD PATTERN
                    current_index = len(data) - 1
                    length = (each.pattern_ABCD_bar_length + each.trade_duration_bars) - 1

                    if take_profit_market_hit or stop_loss_market_hit:

                        each.trade_is_open = False
                        each.trade_is_closed = True  
                        each.trade_exited_date = date(ago=0)

                        if take_profit_market_hit:

                            
                            each.trade_pnl = round(each.trade_reward,2)
                            each.trade_result = 'Win'
                            each.trade_exited_price = each.trade_take_profit
                            each.trade_return_percentage = round((each.trade_reward / each.trade_entered_price) * 100,2)
                        
                        if stop_loss_market_hit:

                            each.trade_pnl = round(each.trade_risk,2)
                            each.trade_result = 'Lost'
                            each.trade_exited_price = each.trade_stop_loss
                            each.trade_return_percentage = round((each.trade_risk / each.trade_entered_price) * 100,2)

                        
                        # GATHER CANDLE IDS OF ABCD PATTERN
                        current_index = len(data) - 1
                        length = (each.pattern_ABCD_bar_length + each.trade_duration_bars) - 1
              
                        each.trade_candle_ids.reverse()

        
        except Exception as e:
            import sys
            print(f'check for exit {e}')
            sys.exit()
      
class ABCD(bt.Strategy):

    # DELCARE INCOMING PARAMS
    params = dict(
        pattern_A = None,
        pattern_AB = None,
        pattern_ABC = None,
        pattern_ABCD = None,
        stockName = None,
        candle_ids=None,
    )
    
    def __init__(self) -> None:      

        # CLEAR PREVIOUS DATA SET
        del chartData[:]
        del allTrades[:]
        del allResults[:]
        del a_pivots[:]
        del pattern_a[:]
        del pattern_ab[:]
        del pattern_abc[:]
        del pattern_abcd[:]

        # INITIALIZE STRATEGY VARIABLES
        self.candle_ids = self.params.candle_ids
        self.counter: int = 0   
        self.dateCanStart: str = None
        self.b_pivots = []
        self.c_pivots= []
        self.d_pivots= []
        self.matched_pivot_ones= []
        self.openTrades: dict = {}
        self.chartData= []
        self.a_pivots= self.params.pattern_A
        self.ab_pivotPairs= self.params.pattern_AB
        self.abc_pivotTrios= self.params.pattern_ABC
        self.pattern_abcd = list = self.params.pattern_ABCD
        self.trade_symbol: str  = self.params.stockName
        self.settings: dict = {
            'candle_ids': self.params.candle_ids,
            'market': 'Bull',
            'pivotLength': int(1),
            'rrr': 1,
            's&r': 'average',

            'inRestrictionArea': True,
            'barsFromBack': True,
            
        }
        self.failed_pivot_trio = []
        self.failed_AB_patterns = []
        self.starting_line = 1
    
    def next(self) -> None:

        def printEachBarDate():
            one = 0
            if(one == 0):
                print(self.datas[0].datetime.date(ago=0),self.data_close[0])

                one+=1
        printEachBarDate()
        
        if self.starting_line >= 3:

            Core().check_for_pivot_A(
                self.datas[0].datetime.date,
                self.data_close,
                self.data_open,
                1,
                self.settings['market'],
                self.a_pivots,
                self.data_high,
                self.data_low,
                self.trade_symbol,
                self.data_volume,
            )
 
            Core().check_for_pivot_B(
                self.a_pivots,
                self.datas[0].datetime.date,
                self.settings['market'],
                self.data_close,
                self.data_open,
                1,
                self.data_low,
                self.data_high,
                self.matched_pivot_ones,
                self.ab_pivotPairs,
                self.failed_AB_patterns,
                self.trade_symbol,
                self.data_volume,
            )
                
            Core().check_for_pivot_C(
                self.ab_pivotPairs,
                self.datas[0].datetime.date,
                self.data_close,
                self.data_open,
                1,
                self.settings,
                self.data_high,
                self.data_low,
                self.settings['market'],
                self.abc_pivotTrios,
                self.trade_symbol,
                self.data_volume
            
            )
                
            Core().check_for_pivot_D(
                self.abc_pivotTrios,
                self.settings['market'],
                self.data_low,
                self.data_high,
                self.datas[0].datetime.date,
                self.data_open,
                self.data_close,
                self.pattern_abcd,
                self.trade_symbol    
            )
                
            Core().enter_trade(
                self.pattern_abcd,
                self.datas[0].datetime.date,
                self.datas[0].volume,
                self.settings['market'],
                self.data_low,
                self.data_high,
                self.settings['rrr'],
            
                )

            Core().exit_trade(
                self.pattern_abcd,
                self.data_open,
                self.data_close,
                self.datas[0].datetime.date,
                self.data,
                'setting1',
                self.trade_symbol


                )

        self.starting_line+=1

    def stop(self):
       
        merged_patterns = StrategyTools().merge_patterns(self.pattern_abcd)    
        sorted_objects = sorted(merged_patterns, key=lambda x: x.pattern_A_pivot_date)

        DataBase().delete_data('pattern_a')
        DataBase().delete_data('pattern_ab')
        DataBase().delete_data('pattern_abc')
        DataBase().delete_data('pattern_abcd')
        
        StrategyTools().load_a_patterns(self.a_pivots)
        StrategyTools().load_ab_patterns(self.ab_pivotPairs)
        StrategyTools().load_abc_patterns(self.abc_pivotTrios)
        # StrategyTools().load_abcd_patterns(sorted_objects)
        StrategyTools().load_abcd_patterns(self.pattern_abcd)

class Pattern_A():


    def __init__(self,
            pattern_A_start_date,
            pattern_A_pivot_date,
            pattern_A_end_date,
            pattern_A_high,
            pattern_A_close,
            pattern_A_open,
            pattern_A_low,
            trade_symbol,
            candles_df
        ):

        self.pattern_A_start_date = pattern_A_start_date
        self.pattern_A_pivot_date = pattern_A_pivot_date
        self.pattern_A_end_date = pattern_A_end_date
        self.pattern_A_open = pattern_A_open
        self.pattern_A_high = pattern_A_high
        self.pattern_A_close = pattern_A_close
        self.pattern_A_low = pattern_A_low
        self.trade_symbol = trade_symbol
        self.pattern_AB_bar_duration = 0
        self.candles = candles_df
        

    def print_data(self):

        print('Pivot Date', self.pattern_A_pivot_date)

class Pattern_AB():
    
    def __init__(self,
            pattern_A_start_date,
            pattern_A_pivot_date, 
            pattern_A_end_date,
            pattern_A_high,
            pattern_A_close,
            pattern_A_open,
            pattern_A_low,
            pattern_B_start_date,
            pattern_B_pivot_date,
            pattern_B_end_date, 
            pattern_B_high,
            pattern_B_close,
            pattern_B_open,
            pattern_B_low,
            trade_symbol,
            pattern_AB_bar_duration, 
            label,
            candles



        ) -> None:
        
        self.id = uuid.uuid4()
        self.trade_symbol = trade_symbol
        self.pattern_BC_bar_duration = 0

        # PATTERN A 
        self.pattern_A_start_date = pattern_A_start_date
        self.pattern_A_pivot_date = pattern_A_pivot_date
        self.pattern_A_end_date = pattern_A_end_date
        self.pattern_A_open = pattern_A_open
        self.pattern_A_high = pattern_A_high
        self.pattern_A_close = pattern_A_close
        self.pattern_A_low = pattern_A_low

        # PATTERN B
        self.pattern_B_start_date = pattern_B_start_date
        self.pattern_B_pivot_date = pattern_B_pivot_date
        self.pattern_B_end_date = pattern_B_end_date
        self.pattern_B_open = pattern_B_open
        self.pattern_B_high = pattern_B_high
        self.pattern_B_close = pattern_B_close
        self.pattern_B_low = pattern_B_low
    
        # PATTERN AB
        # self.pattern_AB_id = pattern_AB_id
        self.pattern_AB_start_date = pattern_A_start_date
        self.pattern_AB_end_date = pattern_B_end_date
        self.pattern_AB_bar_duration = pattern_AB_bar_duration
        
        self.candles = candles
        self.label = label

class Pattern_ABC():
    

    try:
        def __init__(self, 

                # Pattern A
                pattern_A_start_date,
                pattern_A_pivot_date, 
                pattern_A_end_date,
                pattern_A_high,
                pattern_A_close,
                pattern_A_open,
                pattern_A_low,

                # Pattern B
                pattern_B_start_date,
                pattern_B_pivot_date, 
                pattern_B_end_date,
                pattern_B_high,
                pattern_B_close,
                pattern_B_open,
                pattern_B_low,

                # Pattern C
                pattern_C_start_date,
                pattern_C_pivot_date, 
                pattern_C_end_date,
                pattern_C_high,
                pattern_C_close,
                pattern_C_open,
                pattern_C_low,
                
                # PATTERN AB
                pattern_AB_start_date,
                pattern_AB_end_date,
                pattern_AB_bar_length,

                # PATTERN ABC
                pattern_ABC_bar_length,
                pattern_ABC_start_date,
                pattern_ABC_end_date,
                pattern_C_bar_retracment,
                pattern_C_price_retracement,
                pattern_BC_bar_length,
                trade_symbol

            ) -> None:

            self.trade_symbol = trade_symbol
            # PATTERN A 
            # self.pattern_A_id = pattern_A_id
            # # self.pattern_A_pivot_color = pattern_A_pivot_color
            self.pattern_A_start_date = pattern_A_start_date
            self.pattern_A_pivot_date = pattern_A_pivot_date
            self.pattern_A_end_date = pattern_A_end_date
            self.pattern_A_open = pattern_A_open
            self.pattern_A_high = pattern_A_high
            self.pattern_A_close = pattern_A_close
            self.pattern_A_low = pattern_A_low

            # PATTERN B
            # # self.pattern_B_pivot_color = pattern_B_pivot_color
            self.pattern_B_start_date = pattern_B_start_date
            self.pattern_B_pivot_date = pattern_B_pivot_date
            self.pattern_B_end_date = pattern_B_end_date
            self.pattern_B_open = pattern_B_open
            self.pattern_B_high = pattern_B_high
            self.pattern_B_close = pattern_B_close
            self.pattern_B_low = pattern_B_low

            # Pattern C
            # # self.pattern_C_pivot_color = pattern_C_pivot_color
            self.pattern_C_start_date = pattern_C_start_date
            self.pattern_C_pivot_date = pattern_C_pivot_date
            self.pattern_C_end_date = pattern_C_end_date
            self.pattern_C_open = pattern_C_open
            self.pattern_C_high = pattern_C_high
            self.pattern_C_close = pattern_C_close
            self.pattern_C_low = pattern_C_low
        
            # PATTERN AB
            self.pattern_AB_start_date = pattern_AB_start_date
            self.pattern_AB_end_date = pattern_AB_end_date
            self.pattern_AB_bar_length = pattern_AB_bar_length

            # PATTERN ABC
            # self.pattern_ABC_id = pattern_ABC_id
            self.pattern_ABC_start_date = pattern_ABC_start_date
            self.pattern_ABC_end_date = pattern_ABC_end_date
            self.pattern_ABC_bar_length = pattern_ABC_bar_length
            self.pattern_C_bar_retracement = pattern_C_bar_retracment
            self.pattern_C_price_retracement = pattern_C_price_retracement
            self.pattern_BC_bar_length = pattern_BC_bar_length
            self.pattern_ABC_found_D = False

       
    
    except Exception as e:
        print(e)

class Pattern_ABCD():

    def __init__(self, 
        x_pivot_price,
        x_pivot_date,
        d_x_retracement,
        pattern_A_start_date,
        pattern_A_pivot_date, 
        pattern_A_end_date,
        pattern_A_high,
        pattern_A_close,
        pattern_A_open,
        pattern_A_low,
        pattern_B_start_date,
        pattern_B_pivot_date, 
        pattern_B_end_date,
        pattern_B_high,
        pattern_B_close,
        pattern_B_open,
        pattern_B_low,
        pattern_C_start_date,
        pattern_C_pivot_date, 
        pattern_C_end_date,
        pattern_C_high,
        pattern_C_close,
        pattern_C_open,
        pattern_C_low,
        pattern_AB_start_date,
        pattern_AB_end_date,
        pattern_AB_bar_length,
        pattern_ABC_bar_length,
        pattern_ABC_start_date,
        pattern_ABC_end_date,
        pattern_C_bar_retracment,
        pattern_C_price_retracement,
        pattern_BC_bar_length,
        pattern_ABCD_id,
        pattern_ABCD_bar_length,
        pattern_ABCD_start_date,
        pattern_ABCD_end_date,
        pattern_D_bar_retracement,
        pattern_D_price_retracement,
        pattern_CD_bar_length,
        trade_symbol,
        pivot_A_price,
        pivot_B_price,
        pivot_C_price,
        pivot_D_price,
        ab_price_length,
        bc_price_length,
        cd_price_length,
        pivot_D_watchmark,
  

        ) -> None:

        self.x_pivot_price = x_pivot_price
        self.x_pivot_date = x_pivot_date
        self.d_x_retracement = d_x_retracement
        self.trade_symbol = trade_symbol

        # PATTERN A 
        self.pattern_A_start_date = pattern_A_start_date
        self.pattern_A_pivot_date = pattern_A_pivot_date
        self.pattern_A_end_date = pattern_A_end_date
        self.pattern_A_open = pattern_A_open
        self.pattern_A_high = pattern_A_high
        self.pattern_A_close = pattern_A_close
        self.pattern_A_low = pattern_A_low

        # PATTERN B
        self.pattern_B_start_date = pattern_B_start_date
        self.pattern_B_pivot_date = pattern_B_pivot_date
        self.pattern_B_end_date = pattern_B_end_date
        self.pattern_B_open = pattern_B_open
        self.pattern_B_high = pattern_B_high
        self.pattern_B_close = pattern_B_close
        self.pattern_B_low = pattern_B_low

        # Pattern C
        self.pattern_C_start_date = pattern_C_start_date
        self.pattern_C_pivot_date = pattern_C_pivot_date
        self.pattern_C_end_date = pattern_C_end_date
        self.pattern_C_open = pattern_C_open
        self.pattern_C_high = pattern_C_high
        self.pattern_C_close = pattern_C_close
        self.pattern_C_low = pattern_C_low
    
        # PATTERN AB
        self.pattern_AB_start_date = pattern_AB_start_date
        self.pattern_AB_end_date = pattern_AB_end_date
        self.pattern_AB_bar_length = pattern_AB_bar_length

        # PATTERN ABC
        self.pattern_ABC_start_date = pattern_ABC_start_date
        self.pattern_ABC_end_date = pattern_ABC_end_date
        self.pattern_ABC_bar_length = pattern_ABC_bar_length
        self.pattern_C_bar_retracement = pattern_C_bar_retracment
        self.pattern_C_price_retracement = pattern_C_price_retracement
        self.pattern_BC_bar_length = pattern_BC_bar_length

        # PATTERN ABCD
        self.pattern_ABCD_id = pattern_ABCD_id
        self.pattern_ABCD_bar_length = pattern_ABCD_bar_length
        self.pattern_ABCD_start_date = pattern_ABCD_start_date
        self.pattern_ABCD_end_date = pattern_ABCD_end_date
        self.pattern_D_bar_retracement = pattern_D_bar_retracement
        self.pattern_D_price_retracement = pattern_D_price_retracement
        self.pattern_CD_bar_length = pattern_CD_bar_length
        self.candle_ids = []
        self.pattern_d_created_date = None

        # PIVOT PRICES
        self.pivot_A_price = round(pivot_A_price,2)
        self.pivot_B_price = round(pivot_B_price,2)
        self.pivot_C_price = round(pivot_C_price,2)
        self.pivot_D_price = round(pivot_D_price,2)

        # PATTERN PRICE LENGTHS
        self.ab_price_length = round(ab_price_length,2)
        self.bc_price_length = round(bc_price_length,2)
        self.cd_price_length = round(cd_price_length,2)

        self.pivot_D_watchmark = pivot_D_watchmark,
    

        

        # Trade
        self.trade_is_open = False
        self.trade_is_closed = None
        self.trade_entered_date = None
        self.trade_entered_price = None
        self.trade_exited_date = None
        self.trade_exited_price = None
        self.trade_risk = None
        self.trade_reward = None
        self.trade_take_profit = None
        self.trade_stop_loss = None
        self.trade_duration_bars = 0
        self.trade_duration_days = 0
        self.trade_pnl = None
        self.trade_result = None
        self.trade_return_percentage = None
        self.trade_rrr = 1
        self.trade_created = False
        self.trade_candle_ids = []
        self.current_date = None

        self.d_dropped_below_b = None
        self.trade_symbol = trade_symbol

        self.valid = False