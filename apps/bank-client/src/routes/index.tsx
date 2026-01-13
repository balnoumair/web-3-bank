import { Title } from "@solidjs/meta";

export default function Home() {
  const cardStyle = {
    'background-color': '#1f1f1f',
    border: '1px solid rgba(255, 255, 255, 0.06)',
    'border-radius': '1rem',
    padding: '1.5rem',
  };

  const gridStyle = {
    display: 'grid',
    'grid-template-columns': '2fr 1fr',
    gap: '1.5rem',
  };

  const actionCardStyle = {
    'background-color': '#1f1f1f',
    border: '1px solid rgba(255, 255, 255, 0.06)',
    'border-radius': '1rem',
    padding: '1.5rem',
    cursor: 'pointer',
    transition: 'all 0.2s',
  };

  const actionCardGreenStyle = {
    ...actionCardStyle,
    background: 'linear-gradient(135deg, #2d5f4d 0%, #1f4939 100%)',
  };

  const chartContainerStyle = {
    'margin-top': '1.5rem',
    height: '8rem',
    'background-color': '#242424',
    'border-radius': '0.5rem',
    padding: '1rem',
    display: 'flex',
    'align-items': 'flex-end',
    'justify-content': 'space-between',
    gap: '0.5rem',
  };

  const barStyle = (height: string) => ({
    flex: '1',
    height,
    background: 'linear-gradient(to top, rgba(45, 95, 77, 0.5), rgba(45, 95, 77, 0.3))',
    'border-radius': '0.25rem 0.25rem 0 0',
  });

  const activityItemStyle = {
    display: 'flex',
    'align-items': 'center',
    gap: '1rem',
    padding: '0.75rem',
    'border-radius': '0.5rem',
    transition: 'background-color 0.2s',
  };

  const transactions = [
    { emoji: '🍎', name: 'Apple Store, Regent St.', type: 'Electronics', date: '24 Oct, 18:40', amount: -1299.00, status: 'COMPLETED' },
    { emoji: '🔵', name: 'Circle Yield Deposit', type: 'Internal Transfer', date: '23 Oct, 09:15', amount: 5000.00, status: 'COMPLETED' },
    { emoji: '☕', name: 'Artisan Coffee Co.', type: 'Food & Drink', date: '23 Oct, 08:02', amount: -4.50, status: 'PENDING' },
    { emoji: '👤', name: 'Transfer to Sarah M.', type: 'P2P Payment', date: '22 Oct, 18:55', amount: -450.00, status: 'COMPLETED' },
  ];

  const assets = [
    { symbol: '₮', name: 'USDT', fullName: 'Tether USD', amount: 89421.00, change: 2.1, color: '#26a69a' },
    { symbol: '◈', name: 'DAI', fullName: 'Dai Stablecoin', amount: 54120.10, change: 1.8, color: '#f5a623' },
    { symbol: '⬡', name: 'USDC', fullName: 'USD Coin', amount: 105051.00, change: 3.2, color: '#2775ca' },
  ];

  return (
    <main>
      <Title>Dashboard - StableBank</Title>

      <div style={gridStyle}>
        {/* Left Column - Portfolio Card */}
        <div>
          <div style={cardStyle}>
            <div style={{ 'margin-bottom': '1rem' }}>
              <div style={{ 'font-size': '0.875rem', color: '#9ca3af', 'margin-bottom': '0.25rem' }}>
                Total Portfolio Value
              </div>
              <div style={{ display: 'flex', 'align-items': 'baseline', gap: '0.75rem' }}>
                <h2 style={{ 'font-size': '2.5rem', 'font-weight': 'bold', color: 'white' }}>
                  $248,592.10
                </h2>
                <span style={{ 'font-size': '0.875rem', color: '#9ca3af' }}>USDC</span>
                <span style={{ 'font-size': '0.875rem', 'font-weight': '500', color: '#10b981', 'margin-left': 'auto' }}>
                  +14.2%
                </span>
              </div>
            </div>

            {/* Chart */}
            <div style={chartContainerStyle}>
              <div style={barStyle('4rem')}></div>
              <div style={barStyle('3rem')}></div>
              <div style={barStyle('5rem')}></div>
              <div style={barStyle('2.5rem')}></div>
              <div style={barStyle('4.5rem')}></div>
              <div style={barStyle('3.5rem')}></div>
              <div style={barStyle('4.75rem')}></div>
            </div>

            {/* Day labels */}
            <div style={{ display: 'flex', 'justify-content': 'space-between', 'margin-top': '0.5rem', padding: '0 1rem', 'font-size': '0.75rem', color: '#6b7280' }}>
              <span>MON</span>
              <span>TUE</span>
              <span>WED</span>
              <span>THU</span>
              <span>FRI</span>
              <span>SAT</span>
              <span>SUN</span>
            </div>
          </div>

          {/* Recent Activity */}
          <div style={{ ...cardStyle, 'margin-top': '1.5rem' }}>
            <div style={{ display: 'flex', 'align-items': 'center', 'justify-content': 'space-between', 'margin-bottom': '1.5rem' }}>
              <h3 style={{ 'font-size': '1.125rem', 'font-weight': '600', color: 'white' }}>Recent Activity</h3>
              <a href="#" style={{ 'font-size': '0.875rem', color: '#10b981', 'text-decoration': 'none' }}>
                View all history →
              </a>
            </div>

            <div style={{ display: 'flex', 'flex-direction': 'column', gap: '0.5rem' }}>
              {transactions.map((tx) => (
                <div style={activityItemStyle}>
                  <div style={{ width: '2.5rem', height: '2.5rem', 'border-radius': '50%', 'background-color': '#242424', display: 'flex', 'align-items': 'center', 'justify-content': 'center', 'font-size': '1.125rem', 'flex-shrink': '0' }}>
                    {tx.emoji}
                  </div>
                  <div style={{ flex: '1', 'min-width': '0' }}>
                    <div style={{ 'font-weight': '500', color: 'white', 'font-size': '0.875rem' }}>{tx.name}</div>
                    <div style={{ 'font-size': '0.75rem', color: '#6b7280' }}>
                      {tx.type} • {tx.date}
                    </div>
                  </div>
                  <div style={{ 'text-align': 'right', 'flex-shrink': '0' }}>
                    <div style={{ 'font-weight': '600', 'font-size': '0.875rem', color: tx.amount > 0 ? '#10b981' : '#ef4444' }}>
                      {tx.amount > 0 ? '+' : ''}{tx.amount < 0 ? '-' : ''}${Math.abs(tx.amount).toFixed(2)}
                    </div>
                    <div style={{ 'font-size': '0.75rem', 'text-transform': 'uppercase', color: tx.status === 'COMPLETED' ? '#10b981' : '#f59e0b' }}>
                      • {tx.status}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>

        {/* Right Column - Action Buttons */}
        <div style={{ display: 'flex', 'flex-direction': 'column', gap: '0.75rem' }}>
          <div style={actionCardGreenStyle}>
            <div style={{ display: 'flex', 'align-items': 'center', gap: '0.75rem' }}>
              <div style={{ width: '2.5rem', height: '2.5rem', 'border-radius': '0.5rem', 'background-color': 'rgba(255, 255, 255, 0.1)', display: 'flex', 'align-items': 'center', 'justify-content': 'center' }}>
                <svg style={{ width: '1.25rem', height: '1.25rem', color: 'white' }} fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14 5l7 7m0 0l-7 7m7-7H3" />
                </svg>
              </div>
              <div>
                <div style={{ color: 'white', 'font-weight': '600' }}>Send Funds</div>
                <div style={{ 'font-size': '0.875rem', color: 'rgba(255, 255, 255, 0.7)' }}>Transfer USDC to any wallet</div>
              </div>
            </div>
          </div>

          <div style={actionCardStyle}>
            <div style={{ display: 'flex', 'align-items': 'center', gap: '0.75rem' }}>
              <div style={{ width: '2.5rem', height: '2.5rem', 'border-radius': '0.5rem', 'background-color': '#242424', display: 'flex', 'align-items': 'center', 'justify-content': 'center' }}>
                <svg style={{ width: '1.25rem', height: '1.25rem', color: '#9ca3af' }} fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                </svg>
              </div>
              <div>
                <div style={{ color: 'white', 'font-weight': '600' }}>Request Payment</div>
                <div style={{ 'font-size': '0.875rem', color: '#6b7280' }}>Create a payment link or QR</div>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Bottom Section - Your Assets & Stable Savings */}
      <div style={{ display: 'grid', 'grid-template-columns': '1fr 1fr', gap: '1.5rem', 'margin-top': '1.5rem' }}>
        {/* Your Assets */}
        <div style={cardStyle}>
          <h3 style={{ 'font-size': '1.125rem', 'font-weight': '600', color: 'white', 'margin-bottom': '1rem' }}>
            Your Assets
          </h3>
          <div style={{ display: 'flex', 'flex-direction': 'column', gap: '0.75rem' }}>
            {assets.map((asset) => (
              <div style={{ display: 'flex', 'align-items': 'center', 'justify-content': 'space-between', padding: '0.75rem', 'border-radius': '0.5rem', 'background-color': 'rgba(255, 255, 255, 0.02)' }}>
                <div style={{ display: 'flex', 'align-items': 'center', gap: '0.75rem' }}>
                  <div style={{ width: '2rem', height: '2rem', 'border-radius': '50%', 'background-color': `${asset.color}20`, display: 'flex', 'align-items': 'center', 'justify-content': 'center', color: asset.color, 'font-size': '0.875rem', 'font-weight': 'bold' }}>
                    {asset.symbol}
                  </div>
                  <div>
                    <div style={{ 'font-size': '0.875rem', 'font-weight': '500', color: 'white' }}>{asset.name}</div>
                    <div style={{ 'font-size': '0.75rem', color: '#6b7280' }}>{asset.fullName}</div>
                  </div>
                </div>
                <div style={{ 'text-align': 'right' }}>
                  <div style={{ 'font-size': '0.875rem', 'font-weight': '600', color: 'white' }}>
                    ${asset.amount.toLocaleString('en-US', { minimumFractionDigits: 2 })}
                  </div>
                  <div style={{ 'font-size': '0.75rem', color: '#10b981' }}>+{asset.change}%</div>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Stable Savings */}
        <div style={{ ...cardStyle, background: 'linear-gradient(135deg, #2d5f4d 0%, #1f4939 100%)', position: 'relative', overflow: 'hidden' }}>
          <h3 style={{ 'font-size': '1.125rem', 'font-weight': '600', color: 'white', 'margin-bottom': '0.5rem' }}>
            Stable Savings
          </h3>
          <p style={{ 'font-size': '0.875rem', color: 'rgba(255, 255, 255, 0.8)', 'margin-bottom': '1.5rem' }}>
            Earn up to <span style={{ 'font-weight': 'bold', color: 'white' }}>4.5% APY</span> on your USDC holdings with our managed treasury accounts. Institutional-grade security.
          </p>
          <button style={{ padding: '0.625rem 1.25rem', 'background-color': 'rgba(255, 255, 255, 0.2)', color: 'white', border: '1px solid rgba(255, 255, 255, 0.2)', 'border-radius': '0.5rem', 'font-size': '0.875rem', 'font-weight': '500', cursor: 'pointer' }}>
            Learn More
          </button>
          <div style={{ position: 'absolute', right: '-2rem', bottom: '-2rem', width: '8rem', height: '8rem', 'background-color': 'rgba(255, 255, 255, 0.05)', 'border-radius': '50%' }}></div>
        </div>
      </div>
    </main>
  );
}
