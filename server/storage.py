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
            WHERE symbol = 'AAPL'


        ''')
        stored_accounts = pd.read_sql_query(stored_accounts_query, self.engine)

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

    def get_ticker_performance_sql(self, data):
        query = '''
            SELECT 
                symbol,
                COUNT(*) AS count_total,
                SUM(CASE WHEN trade_result = 'Win' THEN 1 ELSE 0 END) AS count_won,
                SUM(CASE WHEN trade_result = 'Lost' THEN 1 ELSE 0 END) AS count_lost,
                SUM(CASE WHEN trade_result = 'Open' THEN 1 ELSE 0 END) AS count_open,
                ROUND(
                    CASE 
                        WHEN SUM(CASE WHEN trade_result IN ('Win', 'Lost') THEN 1 ELSE 0 END) = 0 THEN 0
                        ELSE (
                            SUM(CASE WHEN trade_result = 'Win' THEN 1 ELSE 0 END) * 100.0 /
                            SUM(CASE WHEN trade_result IN ('Win', 'Lost') THEN 1 ELSE 0 END)
                        )
                    END,
                    2
                ) AS win_pct,
                ROUND(AVG(pattern_C_price_retracement), 2) AS retracement_bc_avg,
                ROUND(AVG(pattern_D_price_retracement), 2) AS retracement_cd_avg,
                ROUND(AVG(pattern_AB_bar_length), 2) AS ab_leg_avg,
                ROUND(AVG(pattern_BC_bar_length), 2) AS bc_leg_avg,
                ROUND(AVG(pattern_CD_bar_length), 2) AS cd_leg_avg
            FROM pattern_abcd
            WHERE pattern_C_price_retracement BETWEEN %s AND %s
            AND pattern_D_price_retracement BETWEEN %s AND %s
            AND pattern_AB_bar_length BETWEEN %s AND %s
            AND pattern_BC_bar_length BETWEEN %s AND %s
            AND pattern_CD_bar_length BETWEEN %s AND %s
            GROUP BY symbol
            ORDER BY symbol;
        '''
        
        params = (
            data['bc_retracement_greater'],
            data['bc_retracement_less'],
            data['cd_retracement_greater'],
            data['cd_retracement_less'],
            data['ab_leg_greater'],
            data['ab_leg_less'],
            data['bc_leg_greater'],
            data['bc_leg_less'],
            data['cd_leg_greater'],
            data['cd_leg_less']
        )

        df = pd.read_sql_query(query, self.engine, params=params)
        return df


    

    def get_ticker_peformance(self,patterns):

        def safe_round(val, digits=2, replace_with=0):
            if pd.isna(val) or (isinstance(val, float) and np.isnan(val)):
                return replace_with
            return round(val, digits)


        # Group by ticker and calculate stats
        peformances = []
        grouped = patterns.groupby("symbol")

        for ticker, g in grouped:
            stats = {
                'ticker': ticker,
                'count_total': len(g),
                'count_won': len(g[g['trade_result'] == 'Win']),
                'count_lost': len(g[g['trade_result'] == 'Lost']),
                'count_open': len(g[g['trade_result'] == 'Open']),
                'win_pct': safe_round(
                    (len(g[g['trade_result'] == 'Win']) / len(g)) * 100, 2
                ) if len(g) > 0 else 0,
                'retracement_bc_avg': safe_round(g['pattern_C_price_retracement'].mean()),
                'retracement_cd_avg': safe_round(g['pattern_D_price_retracement'].mean()),
                'ab_leg_avg': safe_round(g['pattern_AB_bar_length'].mean()),
                'bc_leg_avg': safe_round(g['pattern_BC_bar_length'].mean()),
                'cd_leg_avg': safe_round(g['pattern_CD_bar_length'].mean())
            }
            peformances.append(stats)



        return peformances