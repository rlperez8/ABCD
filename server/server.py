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
        
        # stored_accounts = stored_accounts.head(10)
        
   
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
    
        symbol = 'AACG' 
        
        query = '''
            SELECT * FROM pattern_abc
            WHERE trade_symbol = %s
        '''
        df = pd.read_sql_query(query, engine, params=(symbol,))

        df = df.replace({np.nan: None})
        # df = df.head(100)
        df['pattern_A_pivot_date'] = pd.to_datetime(df['pattern_A_pivot_date'])
        df = df[df['pattern_A_pivot_date'] == "2020-02-18"]

        return jsonify({
            "status": "success",
            "data": df.to_dict(orient='records')
        })

    except Exception as e:
        error_message = str(e)
        print('Error:', error_message)
        return jsonify({'error': error_message}), 500

@app.route('/abcd_patterns', methods=['GET'])
def get_abcd_patterns():
    
 
    try:
    
        symbol = 'AAON' 
        
        query = '''
            SELECT * FROM pattern_abcd
            WHERE symbol = %s
        '''
        df = pd.read_sql_query(query, engine, params=(symbol,))

        df = df.replace({np.nan: None})
        
        
        df['pattern_D_price_retracement'] = df['pattern_D_price_retracement'].astype(float)
        df['pattern_AB_bar_length'] = df['pattern_AB_bar_length'].astype(float)
        df['pattern_BC_bar_length'] = df['pattern_BC_bar_length'].astype(float)
        df['pattern_CD_bar_length'] = df['pattern_CD_bar_length'].astype(float)

 
       # Convert to datetime
        df["pattern_A_pivot_date"] = pd.to_datetime(
            df["pattern_A_pivot_date"], 
            format="%a, %d %b %Y %H:%M:%S %Z", 
            errors="coerce"
        )

        # Convert to MM-DD-YYYY format (string)
        df["pattern_A_pivot_date"] = df["pattern_A_pivot_date"].dt.strftime("%m-%d-%Y")
    

     
        statistics = {
            'count_total': len(df),
            'count_won': len(df[df['trade_result'] == 'Win']),
            'count_lost': len(df[df['trade_result'] == 'Lost']),
            'count_open': len(df[df['trade_result'] == 'Open']),  

            'retracement': {
                'bc': {
                    'avg': round(df['pattern_C_price_retracement'].mean(), 2),
                    'min': round(df['pattern_C_price_retracement'].min(), 2),
                    'max': round(df['pattern_C_price_retracement'].max(), 2),
                    'median': round(df['pattern_C_price_retracement'].median(), 2),
                    'std': round(df['pattern_C_price_retracement'].std(), 2)
                },
                'cd':{
                    'avg': round(df['pattern_D_price_retracement'].mean(), 2),
                    'min': round(df['pattern_D_price_retracement'].min(), 2),
                    'max': round(df['pattern_D_price_retracement'].max(), 2),
                    'median': round(df['pattern_D_price_retracement'].median(), 2),
                    'std': round(df['pattern_D_price_retracement'].std(), 2)
                }
            },
            'size':{
                'ab':{
                    'avg': round(df['pattern_AB_bar_length'].mean(), 0),
                    'min': round(df['pattern_AB_bar_length'].min(), 0),
                    'max': round(df['pattern_AB_bar_length'].max(), 0),
                    'median': round(df['pattern_AB_bar_length'].median(), 0),
                    'std': round(df['pattern_AB_bar_length'].std(), 0)
                },
                'bc':{
                    'avg': round(df['pattern_BC_bar_length'].mean(), 0),
                    'min': round(df['pattern_BC_bar_length'].min(), 0),
                    'max': round(df['pattern_BC_bar_length'].max(), 0),
                    'median': round(df['pattern_BC_bar_length'].median(), 0),
                    'std': round(df['pattern_BC_bar_length'].std(), 0)
                },
                'cd':{
                    'avg': round(df['pattern_CD_bar_length'].mean(), 0),
                    'min': round(df['pattern_CD_bar_length'].min(), 0),
                    'max': round(df['pattern_CD_bar_length'].max(), 0),
                    'median': round(df['pattern_CD_bar_length'].median(), 0),
                    'std': round(df['pattern_CD_bar_length'].std(), 0)
                }
            }
        }

        statistics['win_pct'] = (
            round((statistics['count_won'] / statistics['count_total']) * 100, 2)
            if statistics['count_total'] > 0 else 0
        )

        a = df[['pattern_A_start_date', 'pattern_A_pivot_date', 'pattern_A_end_date','pattern_A_open', 'pattern_A_high', 'pattern_A_close', 'pattern_A_low', 'pivot_A_price']]
        b = df[['pattern_B_start_date', 'pattern_B_pivot_date','pattern_B_open', 'pattern_B_high', 'pattern_B_close', 'pattern_B_low', 'pivot_B_price','pattern_B_end_date']]
        c = df[['pattern_C_start_date', 'pattern_C_pivot_date','pattern_C_end_date', 'pattern_C_open', 'pattern_C_high','pattern_C_close', 'pattern_C_low','pattern_C_bar_retracment', 'pivot_C_price','pattern_C_price_retracement']]
        d = df[['pattern_D_bar_retracement','pattern_D_price_retracement','pattern_d_created_date', 'pivot_D_price','d_dropped_below_b']]
        ab = df[['pattern_AB_start_date', 'ab_price_length','pattern_AB_end_date', 'pattern_AB_bar_length']]
        bc = df[['bc_price_length','pattern_BC_bar_length']]
        cd = df[['cd_price_length','pattern_CD_bar_length']]
        abc = df[['pattern_ABC_start_date', 'pattern_ABC_end_date','pattern_ABC_bar_length']]
        abcd = df[['pattern_ABCD_bar_length', 'pattern_ABCD_start_date','pattern_ABCD_end_date']]
        lifecycle = df[[
            'trade_created',
            'trade_entered_date',
            'trade_exited_date',
            'trade_duration_bars',
            'trade_duration_days',
            'trade_is_open',
            'trade_is_closed',
            'valid'
        ]]
        prices = df[[
            'trade_entered_price',
            'trade_exited_price',
            'trade_stop_loss',
            'trade_take_profit'
        ]]
        performance = df[[
            'trade_pnl',
            'trade_result',
            'trade_return_percentage',
            'trade_rrr',
            'trade_risk',
            'trade_reward'
        ]]
        # x = [ 'x_pivot_price', 'x_pivot_date', 'd_x_retracement']


        return jsonify({
            "status": "success",
            "data": df.to_dict(orient='records'),
            'stats': statistics,
            'a': a.to_dict(orient='records'),
            'b': b.to_dict(orient='records'),
            'c': c.to_dict(orient='records'),
            'd': d.to_dict(orient='records'),
            'ab': ab.to_dict(orient='records'),
            'bc': bc.to_dict(orient='records'),
            'cd': cd.to_dict(orient='records'),
            'abc': abc.to_dict(orient='records'),
            'abc': abcd.to_dict(orient='records'),
            'lifecycle': lifecycle.to_dict(orient='records'),
            'prices': prices.to_dict(orient='records'),
            'performance': performance.to_dict(orient='records'),
        })

    except Exception as e:
        error_message = str(e)
        print('Error:', error_message)
        return jsonify({'error': error_message}), 500

@app.route('/filtered_patterns', methods=['POST'])
def filtered_patterns():
    
 
    try:

        data = request.get_json(force=True)['value']
        
        for key, value in data.items():
            print(key,value)
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
        a_date = data['a_date']
        a_date = datetime.strptime(a_date, "%m-%d-%Y").strftime("%Y-%m-%d")
        b_date = data['b_date']
        b_date = datetime.strptime(b_date, "%m-%d-%Y").strftime("%Y-%m-%d")
        c_date = data['c_date']
        c_date = datetime.strptime(c_date, "%m-%d-%Y").strftime("%Y-%m-%d")

        symbol = 'AAON' 
        
        query = '''
            SELECT * 
            FROM pattern_abcd
            WHERE symbol = %s
            AND pattern_C_price_retracement BETWEEN %s AND %s
            AND pattern_D_price_retracement BETWEEN %s AND %s
            AND pattern_AB_bar_length BETWEEN %s AND %s
            AND pattern_BC_bar_length BETWEEN %s AND %s
            AND pattern_CD_bar_length BETWEEN %s AND %s
            AND pattern_A_pivot_date = %s
            AND pattern_B_pivot_date = %s
            AND pattern_C_pivot_date = %s
        '''
        df = pd.read_sql_query(query, engine, params=(
            symbol,
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
            a_date,
            b_date,
            c_date
            
            ))

        df = df.replace({np.nan: None})
        df['pattern_D_price_retracement'] = df['pattern_D_price_retracement'].astype(float)
        df['pattern_AB_bar_length'] = df['pattern_AB_bar_length'].astype(float)
        df['pattern_BC_bar_length'] = df['pattern_BC_bar_length'].astype(float)
        df['pattern_CD_bar_length'] = df['pattern_CD_bar_length'].astype(float)

        # Convert to datetime
        df["pattern_A_pivot_date"] = pd.to_datetime(
            df["pattern_A_pivot_date"], 
            format="%a, %d %b %Y %H:%M:%S %Z", 
            errors="coerce"
        )

        df["pattern_B_pivot_date"] = pd.to_datetime(
            df["pattern_A_pivot_date"], 
            format="%a, %d %b %Y %H:%M:%S %Z", 
            errors="coerce"
        )
        df["pattern_C_pivot_date"] = pd.to_datetime(
            df["pattern_A_pivot_date"], 
            format="%a, %d %b %Y %H:%M:%S %Z", 
            errors="coerce"
        )

        # Convert to MM-DD-YYYY format (string)
        df["pattern_A_pivot_date"] = df["pattern_A_pivot_date"].dt.strftime("%m-%d-%Y")
        df["pattern_B_pivot_date"] = df["pattern_B_pivot_date"].dt.strftime("%m-%d-%Y")
        df["pattern_C_pivot_date"] = df["pattern_C_pivot_date"].dt.strftime("%m-%d-%Y")

        def safe_round(val, digits=2, replace_with=0):
            """Round value safely, replace NaN with given fallback (default=0)."""
            if pd.isna(val) or (isinstance(val, float) and np.isnan(val)):
                return replace_with
            return round(val, digits)
        
        statistics = {
    'count_total': len(df),
    'count_won': len(df[df['trade_result'] == 'Win']),
    'count_lost': len(df[df['trade_result'] == 'Lost']),
    'count_open': len(df[df['trade_result'] == 'Open']),  

    'retracement': {
        'bc': {
            'avg': safe_round(df['pattern_C_price_retracement'].mean(), 2),
            'min': safe_round(df['pattern_C_price_retracement'].min(), 2),
            'max': safe_round(df['pattern_C_price_retracement'].max(), 2),
            'median': safe_round(df['pattern_C_price_retracement'].median(), 2),
            'std': safe_round(df['pattern_C_price_retracement'].std(), 2)
        },
        'cd': {
            'avg': safe_round(df['pattern_D_price_retracement'].mean(), 2),
            'min': safe_round(df['pattern_D_price_retracement'].min(), 2),
            'max': safe_round(df['pattern_D_price_retracement'].max(), 2),
            'median': safe_round(df['pattern_D_price_retracement'].median(), 2),
            'std': safe_round(df['pattern_D_price_retracement'].std(), 2)
        }
    },
    'size': {
        'ab': {
            'avg': safe_round(df['pattern_AB_bar_length'].mean(), 0),
            'min': safe_round(df['pattern_AB_bar_length'].min(), 0),
            'max': safe_round(df['pattern_AB_bar_length'].max(), 0),
            'median': safe_round(df['pattern_AB_bar_length'].median(), 0),
            'std': safe_round(df['pattern_AB_bar_length'].std(), 0)
        },
        'bc': {
            'avg': safe_round(df['pattern_BC_bar_length'].mean(), 0),
            'min': safe_round(df['pattern_BC_bar_length'].min(), 0),
            'max': safe_round(df['pattern_BC_bar_length'].max(), 0),
            'median': safe_round(df['pattern_BC_bar_length'].median(), 0),
            'std': safe_round(df['pattern_BC_bar_length'].std(), 0)
        },
        'cd': {
            'avg': safe_round(df['pattern_CD_bar_length'].mean(), 0),
            'min': safe_round(df['pattern_CD_bar_length'].min(), 0),
            'max': safe_round(df['pattern_CD_bar_length'].max(), 0),
            'median': safe_round(df['pattern_CD_bar_length'].median(), 0),
            'std': safe_round(df['pattern_CD_bar_length'].std(), 0)
        }
    }
}


        statistics['win_pct'] = (
            round((statistics['count_won'] / statistics['count_total']) * 100, 2)
            if statistics['count_total'] > 0 else 0
        )

        
                        
      
     
        return jsonify({
            "status": "success",
            "data": df.to_dict(orient='records'),
            'stats': statistics
        })

    except Exception as e:
        error_message = str(e)
        print('Error:', error_message)
        return jsonify({'error': error_message}), 500



if __name__ == "__main__":

    app.run(debug=True, port=8000)