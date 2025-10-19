from flask import Flask, request, jsonify
import pandas as pd
from flask_cors import CORS, cross_origin
from sqlalchemy import create_engine
import numpy as np 
from datetime import datetime
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

@app.route('/get_selected_ticker', methods=['POST'])
def get_selected_ticker():
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

        print(data)

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

@app.route('/abcd_patterns', methods=['POST'])
def get_abcd_patterns():
    
 
    try:
    
        data = request.get_json(force=True)

        print(data)

        # Extract filters
        symbol = data['symbol']
        
        query = '''
            SELECT * FROM pattern_abcd
            WHERE symbol = %s
        '''
        df = pd.read_sql_query(query, engine, params=(symbol,))

        df = df.replace({np.nan: None})
        

        return jsonify({
            "status": "success",
            "data": df.to_dict(orient='records'),
      
        })

    except Exception as e:
        error_message = str(e)
        print('Error:', error_message)
        return jsonify({'error': error_message}), 500


@app.route('/filtered_patterns', methods=['POST'])
def filtered_patterns():
    try:
        data = request.get_json(force=True)['value']

 

        # Extract filters
        cd_retracement_greater = data['cd_retracement_greater']
        cd_retracement_less = data['cd_retracement_less']
        bc_retracement_greater = data['bc_retracement_greater']
        bc_retracement_less = data['bc_retracement_less']

        ab_leg_greater = data['ab_leg_greater']
        ab_leg_less = data['ab_leg_less']
        bc_leg_greater = data['bc_leg_greater']
        bc_leg_less = data['bc_leg_less']
        cd_leg_greater = data['cd_leg_greater']
        cd_leg_less = data['cd_leg_less']

        # a_date = datetime.strptime(data['a_date'], "%m-%d-%Y").strftime("%Y-%m-%d")
        # b_date = datetime.strptime(data['b_date'], "%m-%d-%Y").strftime("%Y-%m-%d")
        # c_date = datetime.strptime(data['c_date'], "%m-%d-%Y").strftime("%Y-%m-%d")

        # Query all patterns that match filters across ALL tickers
        query = '''
            SELECT * 
            FROM pattern_abcd
            WHERE pattern_C_price_retracement BETWEEN %s AND %s
            AND pattern_D_price_retracement BETWEEN %s AND %s
            AND pattern_AB_bar_length BETWEEN %s AND %s
            AND pattern_BC_bar_length BETWEEN %s AND %s
            AND pattern_CD_bar_length BETWEEN %s AND %s
       
        '''
        df = pd.read_sql_query(query, engine, params=(
            bc_retracement_greater,
            bc_retracement_less,
            cd_retracement_greater,
            cd_retracement_less,
            ab_leg_greater,
            ab_leg_less,
            bc_leg_greater,
            bc_leg_less,
            cd_leg_greater,
            cd_leg_less,
    
        ))



        if df.empty:
            return jsonify({"status": "success", "data": [], "stats": []})

        df = df.replace({np.nan: None})
    

        # Helper to round safely
        def safe_round(val, digits=2, replace_with=0):
            if pd.isna(val) or (isinstance(val, float) and np.isnan(val)):
                return replace_with
            return round(val, digits)

        # Group by ticker and calculate stats
        results = []
        grouped = df.groupby("symbol")

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
            results.append(stats)

        unique_symbols = df['symbol'].unique().tolist()
     
        # return jsonify({"status": "success", "data": df.to_dict(orient='records'), "stats": []})

        return jsonify({
            "status": "success",
            "data": results
        })

    except Exception as e:
        print("Error:", e)
        return jsonify({'error': str(e)}), 500
    
# from sqlalchemy import create_engine, text
# @app.route('/ticker_peformances', methhods=['GET'])
# def ticker_performance():

    query = '''
        SELECT * FROM pattern_abcd

    '''
    df = pd.read_sql_query(query, engine, params=())


    print(df)

if __name__ == "__main__":

    app.run(debug=True, port=8000)