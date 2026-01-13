import { Component } from 'solid-js';

const Sidebar: Component = () => {
    const sidebarStyle = {
        position: 'fixed',
        left: '0',
        top: '0',
        height: '100vh',
        width: '13rem',
        'background-color': '#151515',
        'border-right': '1px solid rgba(255, 255, 255, 0.05)',
        display: 'flex',
        'flex-direction': 'column',
    };

    const logoContainerStyle = {
        padding: '1.5rem',
        'border-bottom': '1px solid rgba(255, 255, 255, 0.05)',
    };

    const logoStyle = {
        display: 'flex',
        'align-items': 'center',
        gap: '0.75rem',
    };

    const iconStyle = {
        width: '2rem',
        height: '2rem',
        'background-color': 'white',
        'border-radius': '0.375rem',
        display: 'flex',
        'align-items': 'center',
        'justify-content': 'center',
        color: 'black',
        'font-size': '1.25rem',
        'font-weight': 'bold',
    };

    const navStyle = {
        flex: '1',
        padding: '1rem',
        display: 'flex',
        'flex-direction': 'column',
        gap: '0.25rem',
    };

    const navItemActiveStyle = {
        display: 'flex',
        'align-items': 'center',
        gap: '0.75rem',
        padding: '0.75rem 1rem',
        'border-radius': '0.5rem',
        background: 'linear-gradient(to right, #2d5f4d, #3a7360)',
        color: 'white',
        'text-decoration': 'none',
        'font-size': '0.875rem',
        'font-weight': '500',
    };

    const navItemStyle = {
        display: 'flex',
        'align-items': 'center',
        gap: '0.75rem',
        padding: '0.75rem 1rem',
        'border-radius': '0.5rem',
        color: '#9ca3af',
        'text-decoration': 'none',
        'font-size': '0.875rem',
        'font-weight': '500',
        transition: 'all 0.2s',
    };

    const bottomStyle = {
        padding: '1rem',
        'border-top': '1px solid rgba(255, 255, 255, 0.05)',
    };

    const buttonStyle = {
        width: '100%',
        padding: '0.625rem 1rem',
        background: 'linear-gradient(to right, #2d5f4d, #3a7360)',
        color: 'white',
        'border-radius': '0.5rem',
        'font-size': '0.875rem',
        'font-weight': '500',
        border: 'none',
        cursor: 'pointer',
    };

    return (
        <div style={sidebarStyle}>
            {/* Logo */}
            <div style={logoContainerStyle}>
                <div style={logoStyle}>
                    <div style={iconStyle}>$</div>
                    <span style={{ color: 'white', 'font-weight': '600' }}>StableBank</span>
                </div>
            </div>

            {/* Navigation */}
            <nav style={navStyle}>
                <a href="/" style={navItemActiveStyle}>
                    <span style={{ 'font-size': '1.125rem' }}>📊</span>
                    <span>Dashboard</span>
                </a>

                <a href="#" style={navItemStyle}>
                    <span style={{ 'font-size': '1.125rem' }}>💳</span>
                    <span>Wallet</span>
                </a>

                <a href="#" style={navItemStyle}>
                    <span style={{ 'font-size': '1.125rem' }}>📋</span>
                    <span>Activity</span>
                </a>

                <a href="#" style={navItemStyle}>
                    <span style={{ 'font-size': '1.125rem' }}>📈</span>
                    <span>Yield</span>
                </a>

                <a href="#" style={navItemStyle}>
                    <span style={{ 'font-size': '1.125rem' }}>⚙️</span>
                    <span>Settings</span>
                </a>
            </nav>

            {/* Bottom Section */}
            <div style={bottomStyle}>
                <div style={{ 'margin-bottom': '0.75rem' }}>
                    <div style={{ 'font-size': '0.75rem', color: '#6b7280', 'text-transform': 'uppercase', 'margin-bottom': '0.25rem' }}>
                        Account Tier
                    </div>
                    <div style={{ display: 'flex', 'align-items': 'center', gap: '0.5rem' }}>
                        <div style={{ width: '0.5rem', height: '0.5rem', 'border-radius': '50%', 'background-color': '#10b981' }}></div>
                        <span style={{ 'font-size': '0.875rem', color: 'white', 'font-weight': '500' }}>Premium Pro</span>
                    </div>
                </div>

                <button style={buttonStyle}>
                    Connect Wallet
                </button>
            </div>
        </div>
    );
};

export default Sidebar;
