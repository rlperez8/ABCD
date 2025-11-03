from flask import Flask, request, jsonify
import pandas as pd
from flask_cors import CORS, cross_origin
from sqlalchemy import create_engine
import numpy as np 
from datetime import datetime

from storage import *

app = Flask(__name__)
CORS(app)

# Replace with your actual MySQL database URL
database_url = "mysql+pymysql://rperezkc:Nar8uto!@localhost:3306/abcd"
engine = create_engine(database_url)

@app.route('/typed_ticker', methods=['POST', 'OPTIONS'])
@cross_origin() 
def typed_ticker():
    try:
        if request.method == 'OPTIONS':
            return '', 200  # For preflight request, return empty response with status 200

        # Get the data from the request
        data = request.get_json()
  

        typed_ticker = data.get('typed_ticker', '').strip()


        if not typed_ticker:
            return jsonify({"error": "Missing 'typed_ticker' in request"}), 400

        # SQL query using 'LIKE' for matching the beginning of 'symbol'
        query = '''
            SELECT * FROM listing_status
            WHERE symbol LIKE %s
        '''
        
        # Add '%' around typed_ticker to match any symbol starting with typed_ticker
        search_pattern = f"{typed_ticker}%"

        # Pass the search pattern as a tuple for the query parameter
        stored_accounts = pd.read_sql_query(query, engine, params=(search_pattern,))

        # Return results
        return jsonify({
            "status": "success",
            "data": stored_accounts.to_dict(orient='records')
        })

    except Exception as e:
        print("Error:", str(e))
        return jsonify({"error": str(e)}), 500

@app.route('/get_candles', methods=['POST'])
def get_candles():
    try:
        data = request.get_json()

        symbol = data.get('symbol', '').strip()
      
        if not symbol:
            return jsonify({"error": "Missing 'symbol' in request"}), 400
        
        stored_accounts_query = f'''
            SELECT * FROM candles
            WHERE symbol = '{symbol}'
        '''
        stored_accounts = pd.read_sql_query(stored_accounts_query, engine)
        
        # Optionally, format floats in the DataFrame
        stored_accounts = stored_accounts.round(2)
        stored_accounts.rename(columns={
            'date':'candle_date',
            'high':'candle_high',
            'close':'candle_close',
            'open':'candle_open',
            'low':'candle_low',

        },inplace=True)
        

   
        return jsonify({"status": "success", "data": stored_accounts.to_dict(orient='records')})

    except Exception as e:
        # Handle errors gracefully
        error_message = str(e)
        print(f"Error: {error_message}")
        return jsonify({"error": error_message}), 500

@app.route('/get_listed_tickers', methods=['POST'])
def get_listed_tickers():
    try:
        data = request.get_json()

        search = data['searched']
        type = data['type']

        def create_query(searched_ticker, asset_type):

            base_query = "SELECT * FROM listing_status"
            where_clauses = []
            params = []

            if asset_type.lower() != "all":
                where_clauses.append("assetType = %s")
                params.append(asset_type)

            # Apply search term filter only if it's provided
            if searched_ticker:
              
                where_clauses.append("(symbol LIKE %s OR name LIKE %s)")
                keyword = f"%{searched_ticker}%"
                params.extend((keyword, keyword))

            # Add WHERE clause if needed
            if where_clauses:
                base_query += " WHERE " + " AND ".join(where_clauses)

            return base_query, params
        
        base_query, params = create_query(search, type)

        params = tuple(params)
        all_tickers = pd.read_sql_query(sql=base_query, con=engine, params=params)
        all_tickers = all_tickers.head(100)

    
        return jsonify({"status": "success", "data": all_tickers.to_dict(orient='records')})

    except Exception as e:
        # Handle errors gracefully
        error_message = str(e)
        print(f"Error: {error_message}")
        return jsonify({"error": error_message}), 500

@app.route('/get_abcd_of_selected_symbol', methods=['POST'])
def get_abcd_of_selected_symbol():
    try:
        data = request.get_json(force=True)
        symbol = data.get('symbol')
        if not symbol:
            return jsonify({'error': 'Missing "symbol" in request body.'}), 400

        query = '''
            SELECT * FROM pattern_abcd
            WHERE symbol = %s
        '''
        df = pd.read_sql_query(query, engine, params=(symbol,))

        # Replace NaN with None (null in JSON)
        df = df.replace({np.nan: None})

        return jsonify({
            "status": "success",
            "data": df.to_dict(orient='records')
        })

    except Exception as e:
        error_message = str(e)
        print('Error:', error_message)
        return jsonify({'error': error_message}), 500

@app.route('/ab_candles', methods=['GET'])
def get_ab_candles():
    try:
    
        symbol = 'AACG' 
        
        query = '''
            SELECT * FROM pattern_ab
            WHERE trade_symbol = %s
        '''
        df = pd.read_sql_query(query, engine, params=(symbol,))

        df = df.replace({np.nan: None})
        # df['pattern_A_start_date'] = pd.to_datetime(df['pattern_A_start_date'], errors='coerce')
        # df = df[
        #     # (df['pattern_A_start_date'].dt.year == 2021) &
        #     (df['pattern_A_start_date'].dt.month == 2)
        # ]
        
        # print(df)
        # df = df.iloc[0:100]

        return jsonify({
            "status": "success",
            "data": df.to_dict(orient='records')
        })

    except Exception as e:
        error_message = str(e)
        print('Error:', error_message)
        return jsonify({'error': error_message}), 500

@app.route('/abc_patterns', methods=['GET'])
def get_abc_patterns():
    
    try:

        data = request.get_json(force=True)['value']

        # Extract filters
        symbol = data['symbol']
    

        
        query = '''
            SELECT * FROM pattern_abc
            WHERE trade_symbol = %s
        '''
        df = pd.read_sql_query(query, engine, params=(symbol))

        df = df.replace({np.nan: None})
   
        return jsonify({
            "status": "success",
            "data": df.to_dict(orient='records')
        })

    except Exception as e:
        error_message = str(e)
        print('Error:', error_message)
        return jsonify({'error': error_message}), 500

@app.route('/filtered_patterns', methods=['POST'])
def filtered_patterns():
    try:
        data = request.get_json(force=True)

        # Extract filters safely
        symbol = data['symbol']
        filter_ = data['filter']

        query = '''
            SELECT *
            FROM pattern_abcd
            WHERE pattern_C_price_retracement BETWEEN %s AND %s
              AND pattern_D_price_retracement BETWEEN %s AND %s
              AND pattern_AB_bar_length BETWEEN %s AND %s
              AND pattern_BC_bar_length BETWEEN %s AND %s
              AND pattern_CD_bar_length BETWEEN %s AND %s
              AND pattern_BC_bar_length BETWEEN 0.38 * pattern_AB_bar_length AND 0.62 * pattern_AB_bar_length
              AND pattern_CD_bar_length BETWEEN 1.0 * pattern_AB_bar_length AND 1.27 * pattern_AB_bar_length
              AND symbol = %s
        '''

        params = (
            filter_['bc_retracement_greater'],
            filter_['bc_retracement_less'],
            filter_['cd_retracement_greater'],
            filter_['cd_retracement_less'],
            filter_['ab_leg_greater'],
            filter_['ab_leg_less'],
            filter_['bc_leg_greater'],
            filter_['bc_leg_less'],
            filter_['cd_leg_greater'],
            filter_['cd_leg_less'],
            symbol
        )

        df = pd.read_sql_query(query, engine, params=params)
        df = df.replace({np.nan: None})
     
        df['pattern_A_pivot_date'] = df['pattern_A_pivot_date'].astype(str)

        return jsonify({
            "status": "success",
            "data": df.to_dict(orient='records'),
        })

    except Exception as e:
        print('Error:', e)
        return jsonify({'error': str(e)}), 500

@app.route('/filtered_peformances', methods=['POST'])
def filtered_peformances():
    try:
        data = request.get_json(force=True)['value']
        db = DataBase()
        filtered_abcd_patterns = db.get_ticker_performance_sql(data)
     
        avg_colume = db.get_ticker_average_volume()

        df = pd.merge(filtered_abcd_patterns, avg_colume, on='symbol', how='inner')

        df.rename(columns={'symbol': 'ticker'}, inplace=True)


        return jsonify({
            "status": "success",
            "data": df.to_dict(orient='records')
        })

    except Exception as e:
        print("Error:", e)
        return jsonify({'error': str(e)}), 500
@app.route('/recent_patterns', methods=['POST'])
def recent_patterns():
    try:
        data = request.get_json()
        bc_retracement_greater = data['bc_retracement_greater']
        bc_retracement_less = data['bc_retracement_less']
        cd_retracement_greater = data['cd_retracement_greater']
        cd_retracement_less = data['cd_retracement_less']

        query = '''
SELECT p.*
FROM pattern_abcd p
JOIN support_and_resistance s
    ON p.symbol = s.symbol
    AND p.trade_entered_price BETWEEN 0.99 * s.price AND 1.01 * s.price
WHERE p.trade_entered_date >= %s
    AND p.pattern_C_price_retracement BETWEEN %s AND %s
    AND p.pattern_D_price_retracement BETWEEN %s AND %s
        '''


  

        params = (
            "2025-10-01",
            bc_retracement_greater,
            bc_retracement_less,
            cd_retracement_greater,
            cd_retracement_less
        )

        with engine.connect() as conn:
            df = pd.read_sql_query(query, conn, params=params)

        df = df.replace({np.nan: None})

        return jsonify({
            'status': 'success',
            'data': df.to_dict(orient='records')
        })

    except Exception as e:
        print(e)
        return jsonify({
            'status': 'error',
            'message': str(e)
        }), 500

    except Exception as e:
        print(e)
        return jsonify({
            'status': 'error',
            'message': str(e)
        }), 500

@app.route('/add_watchlist', methods=['POST'])
def add_watchlist():

    try:
        data = request.get_json(force=True)
    
        wl_name = data.get('wl_name') 
        df = pd.DataFrame([{"wl_name": wl_name}])
        
     
        
        df.to_sql('watchlist', engine, index=False, if_exists='append')
    
        return jsonify({
            "status": "success",
     
        })


    except Exception as e:
        print(e)

@app.route('/get_all_watchlist', methods=['GET'])
def get_all_watchlist():
    
    try:

        query = '''
            SELECT * FROM watchlist
        '''
        df = pd.read_sql_query(query, engine, params=())

        
   
        return jsonify({
            "status": "success",
            "data": df.to_dict(orient='records')
        })

    except Exception as e:
        error_message = str(e)
        print('Error:', error_message)
        return jsonify({'error': error_message}), 500

@app.route('/delete_watchlist', methods=['POST'])
def delete_watchlist():
    try:
        data = request.get_json(force=True)
        wl_name = data.get('wl_name', '').strip()

        if not wl_name:
            return jsonify({'error': 'wl_name is required'}), 400

        delete_query = "DELETE FROM watchlist WHERE wl_name = :wl_name"

        with engine.begin() as conn:
            # Check if it exists first
            result = conn.execute(text("SELECT * FROM watchlist WHERE wl_name = :wl_name"), {"wl_name": wl_name}).fetchall()
            print("Before delete:", result)

            if not result:
                return jsonify({'error': f'Watchlist "{wl_name}" not found'}), 404

            conn.execute(text(delete_query), {"wl_name": wl_name})

            updated_df = pd.read_sql_query("SELECT * FROM watchlist", engine)
            print("After delete:", updated_df)

        return jsonify({
            "status": "success",
            "deleted": wl_name,
            "watchlist": updated_df.to_dict(orient='records')
        })

    except Exception as e:
        print("Error:", str(e))
        return jsonify({'error': str(e)}), 500

@app.route('/add_pattern_to_a_watchlist', methods=['POST'])
def add_pattern_to_a_watchlist():
    try:
        data = request.get_json(force=True)
  
        wl_name = data.get('wl_name') 
        pattern_id = data.get('pattern_id')

      
        df = pd.DataFrame([{
            'wl_name': wl_name,
            'pattern_id': pattern_id
        }])
     
        # List of date columns that need conversion
        date_columns = [
            "pattern_A_pivot_date",
            "pattern_B_pivot_date",
            "pattern_C_pivot_date",
            "pattern_C_start_date",
            "pattern_C_end_date",
            "trade_entered_date",
            "trade_exited_date",
            # add any other date columns here
        ]

        for col in date_columns:
            if col in df.columns:
                # convert to datetime, errors='coerce' will turn invalid dates into NaT
                df[col] = pd.to_datetime(df[col], errors='coerce')
                # convert to MySQL-friendly string format
                df[col] = df[col].dt.strftime('%Y-%m-%d %H:%M:%S')
                

        df.to_sql('watchlist_patterns', engine, index=False, if_exists='append')

      
        df = df.replace({np.nan: None})
        return jsonify({
            "status": "success",
     
        })

    except Exception as e:
        print("Error:", str(e))
        return jsonify({'error': str(e)}), 500
    
@app.route('/get_all_patterns_in_watchlist', methods=['GET'])
def get_all_patterns_in_watchlist():
    
    try:

        query = '''
            SELECT * FROM watchlist_patterns
        '''
        df_watchlist = pd.read_sql_query(query, engine, params=())
        pattern_ids = df_watchlist['pattern_id'].tolist() 

        query = text("SELECT * FROM pattern_abcd WHERE id IN :ids")
        df = pd.read_sql_query(query, engine, params={"ids": tuple(pattern_ids)})
        df = df.replace({np.nan: None})
   
        return jsonify({
            "status": "success",
            "data": df.to_dict(orient='records')
        })

    except Exception as e:
        error_message = str(e)
        print('Error:', error_message)
        return jsonify({'error': error_message}), 500

@app.route('/monthly_peformance', methods=['POST'])
def monthly_peformance():
    try:
        data = request.get_json()

        bc_retracement_greater = data['bc_retracement_greater']
        bc_retracement_less = data['bc_retracement_less']
        cd_retracement_greater = data['cd_retracement_greater']
        cd_retracement_less = data['cd_retracement_less']
        query = '''
            SELECT 
                EXTRACT(YEAR FROM trade_entered_date) AS year,
                EXTRACT(MONTH FROM trade_entered_date) AS month,
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
                ) AS win_pct
            FROM pattern_abcd p
            WHERE 
                EXTRACT(YEAR FROM p.trade_entered_date) = 2025
                AND EXTRACT(MONTH FROM p.trade_entered_date) >= 10
                AND p.pattern_BC_bar_length BETWEEN 0.38 * p.pattern_AB_bar_length AND 0.62 * p.pattern_AB_bar_length
                AND p.pattern_CD_bar_length BETWEEN 1.0 * p.pattern_AB_bar_length AND 1.27 * p.pattern_AB_bar_length
                AND p.pattern_C_price_retracement BETWEEN %s AND %s
                AND p.pattern_D_price_retracement BETWEEN %s AND %s
                AND EXISTS (
                    SELECT 1
                    FROM support_and_resistance s
                    WHERE s.symbol = p.symbol
                        AND (
                            p.trade_entered_price BETWEEN 0.99 * s.price AND 1.01 * s.price
                         
                        )
                )
            GROUP BY year, month
            ORDER BY year, month;
        '''



        params = (
            bc_retracement_greater,
            bc_retracement_less,
            cd_retracement_greater,
            cd_retracement_less,
        )

        df = pd.read_sql_query(query, engine, params=params)

        return jsonify({
            "status": "success",
            "data": df.to_dict(orient='records')
        })

    except Exception as e:
        print("Error:", e)
        return jsonify({'error': str(e)}), 500

@app.route('/get_support_resistance_lines', methods=['POST'])
def get_support_resistance_lines():
    try:
        data = request.get_json()
        symbol = data.get('symbol')
        
        if not symbol:
            return jsonify({"status": "error", "message": "Symbol is required"}), 400

        query = '''
            SELECT * FROM support_and_resistance
            WHERE symbol = %s
        '''
        df = pd.read_sql_query(query, engine, params=(symbol,))

        if df.empty:
            return jsonify({"status": "error", "message": "No support/resistance data found for this symbol"}), 404

        return jsonify({
            "status": "success",
            "symbol": symbol,
            "data": df.to_dict(orient='records')
        })

    except Exception as e:
        return jsonify({"status": "error", "message": str(e)}), 500

if __name__ == "__main__":

    app.run(debug=True, port=8000)