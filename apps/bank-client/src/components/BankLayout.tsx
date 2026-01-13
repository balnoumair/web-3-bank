import { Component, JSX } from 'solid-js';
import Sidebar from './Sidebar';

interface BankLayoutProps {
    children: JSX.Element;
}

const BankLayout: Component<BankLayoutProps> = (props) => {
    const topBarStyle = {
        height: '4rem',
        'border-bottom': '1px solid rgba(255, 255, 255, 0.05)',
        'background-color': '#0f0f0f',
        display: 'flex',
        'align-items': 'center',
        'justify-content': 'space-between',
        padding: '0 2rem',
    };

    const searchContainerStyle = {
        position: 'relative' as const,
        width: '100%',
        'max-width': '28rem',
    };

    const searchInputStyle = {
        width: '100%',
        padding: '0.625rem 1rem 0.625rem 2.5rem',
        'background-color': '#242424',
        border: '1px solid rgba(255, 255, 255, 0.06)',
        'border-radius': '0.5rem',
        color: 'white',
        'font-size': '0.875rem',
        outline: 'none',
    };

    const searchIconStyle = {
        position: 'absolute' as const,
        left: '0.75rem',
        top: '50%',
        transform: 'translateY(-50%)',
        color: '#6b7280',
        width: '1rem',
        height: '1rem',
    };

    const rightSectionStyle = {
        display: 'flex',
        'align-items': 'center',
        gap: '0.75rem',
    };

    const iconButtonStyle = {
        padding: '0.5rem',
        'background-color': '#242424',
        border: '1px solid rgba(255, 255, 255, 0.06)',
        'border-radius': '0.5rem',
        color: '#9ca3af',
        cursor: 'pointer',
        display: 'flex',
        'align-items': 'center',
        'justify-content': 'center',
    };

    const profileStyle = {
        display: 'flex',
        'align-items': 'center',
        gap: '0.75rem',
        'padding-left': '0.75rem',
        'border-left': '1px solid rgba(255, 255, 255, 0.06)',
    };

    const avatarStyle = {
        width: '2.5rem',
        height: '2.5rem',
        'border-radius': '50%',
        background: 'linear-gradient(to bottom right, #fb923c, #ea580c)',
        display: 'flex',
        'align-items': 'center',
        'justify-content': 'center',
        color: 'white',
        'font-weight': '600',
    };

    return (
        <div style={{ 'min-height': '100vh', 'background-color': '#0f0f0f' }}>
            <Sidebar />

            {/* Main Content Area */}
            <div style={{ 'margin-left': '13rem' }}>
                {/* Top Navigation */}
                <header style={topBarStyle}>
                    {/* Search Bar */}
                    <div style={searchContainerStyle}>
                        <svg style={searchIconStyle} fill="none" viewBox="0 0 24 24" stroke="currentColor">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                        </svg>
                        <input
                            type="text"
                            placeholder="Search transactions, assets..."
                            style={searchInputStyle}
                        />
                    </div>

                    {/* Right Side */}
                    <div style={rightSectionStyle}>
                        {/* Notification Bell */}
                        <button style={iconButtonStyle}>
                            <svg style={{ width: '1.25rem', height: '1.25rem' }} fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9" />
                            </svg>
                        </button>

                        {/* User Profile */}
                        <div style={profileStyle}>
                            <div style={{ 'text-align': 'right' }}>
                                <div style={{ 'font-size': '0.875rem', 'font-weight': '500', color: 'white' }}>
                                    Alex Sterling
                                </div>
                                <div style={{ 'font-size': '0.75rem', color: '#6b7280' }}>
                                    06/16 - 4521
                                </div>
                            </div>
                            <div style={avatarStyle}>AS</div>
                        </div>
                    </div>
                </header>

                {/* Page Content */}
                <main style={{ padding: '2rem' }}>
                    {props.children}
                </main>
            </div>
        </div>
    );
};

export default BankLayout;
