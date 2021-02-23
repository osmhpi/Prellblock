use super::{AccountMeta, CA};
use dialoguer::{theme::Theme, Input, Password, Select};
use openssl::{
    asn1::{Asn1Integer, Asn1Time},
    bn::{BigNum, MsbOption},
    ec::{EcGroup, EcKey},
    error::ErrorStack,
    hash::MessageDigest,
    nid::Nid,
    pkcs12::Pkcs12,
    pkey::{PKey, Private},
    x509::{
        extension::{
            BasicConstraints, ExtendedKeyUsage, KeyUsage, SubjectAlternativeName,
            SubjectKeyIdentifier,
        },
        X509NameBuilder, X509NameRef, X509,
    },
};
use prellblock_client_api::account::{Account, AccountType};
use std::{
    collections::HashSet,
    fs,
    net::IpAddr,
    path::Path,
    time::{Duration, SystemTime},
};

pub(super) fn handle_create_certificates<'a>(
    theme: &'a dyn Theme,
    accounts: &mut Vec<AccountMeta>,
    ca: &mut Option<CA>,
) {
    let certificate_actions = [
        "Create CA certificate",
        "Load CA certificate",
        "Create TLS keys and certificates for all RPUs",
        "Go Back",
    ];
    let mut certificate_selection = Select::with_theme(theme);
    certificate_selection
        .with_prompt("Please select an action:")
        .items(&certificate_actions)
        .default(1);

    loop {
        match certificate_selection.interact().unwrap() {
            0 => match create_ca_cert(theme) {
                Ok((cert, pkey)) => {
                    *ca = Some(CA {
                        cert,
                        pkey,
                        created: true,
                    });
                    certificate_selection.default(2);
                }
                Err(err) => panic!(
                    "Failed to create Certificate Authority Certificate: {}",
                    err
                ),
            },
            1 => load_ca_cert(theme, ca),
            2 => {
                if ca.is_none() {
                    println!("No CA certificate (create or load beforehand!).");
                    continue;
                }
                let ca = ca.as_ref().unwrap();

                // Filter all RPUs and create certs based on name.
                let rpus = accounts.iter_mut().filter(|meta| {
                    if let AccountType::RPU { .. } = meta.account.account_type {
                        true
                    } else {
                        false
                    }
                });

                for rpu_meta in rpus {
                    let (cert, key) = create_rpu_cert(ca, &rpu_meta.account).unwrap();
                    let pkcs = Pkcs12::builder()
                        .build("prellblock", "cert", &key, &cert)
                        .unwrap();
                    rpu_meta.rpu_cert = Some(pkcs);
                }
                certificate_selection.default(2);
            }
            3 => break,
            _ => panic!("Invalid selection."),
        }
    }
}

fn create_ca_cert(theme: &'_ dyn Theme) -> Result<(X509, PKey<Private>), ErrorStack> {
    let country = Input::<String>::with_theme(theme)
        .with_prompt("Enter Country Code:")
        .default("DE".to_string())
        .interact()
        .unwrap();

    let state = Input::<String>::with_theme(theme)
        .with_prompt("Enter State / Province:")
        .default("Brandenburg".to_string())
        .interact()
        .unwrap();

    let organization = Input::<String>::with_theme(theme)
        .with_prompt("Enter Organization:")
        .default("Acme Corporation".to_string())
        .interact()
        .unwrap();

    let common_name = Input::<String>::with_theme(theme)
        .with_prompt("Enter Common Name:")
        .default("acme-ca".to_string())
        .interact()
        .unwrap();

    let valid_for = Input::<u32>::with_theme(theme)
        .with_prompt("Enter validity in days from today (CAUTION: some systems only support a maximum of around 365 days):")
        .default(365)
        .interact()
        .unwrap();

    let days = Duration::from_secs(60 * 60 * 24 * (valid_for as u64));
    let info = SystemTime::now().checked_add(days).unwrap();
    println!(
        "The CA certificate is valid until: {}.",
        humantime::format_rfc3339(info)
    );

    let valid_for = Asn1Time::days_from_now(valid_for)?;
    let now = Asn1Time::days_from_now(0)?;

    let pkey = generate_private_key()?;

    let mut x509_name = X509NameBuilder::new()?;
    x509_name.append_entry_by_nid(Nid::COUNTRYNAME, &country)?;
    x509_name.append_entry_by_nid(Nid::STATEORPROVINCENAME, &state)?;
    x509_name.append_entry_by_nid(Nid::ORGANIZATIONNAME, &organization)?;
    x509_name.append_entry_by_nid(Nid::COMMONNAME, &common_name)?;
    let x509_name = x509_name.build();

    let mut x509 = X509::builder()?;
    x509.set_not_after(&valid_for)?;
    x509.set_not_before(&now)?;
    x509.set_subject_name(&x509_name)?;
    x509.set_issuer_name(&x509_name)?;
    x509.set_version(2)?; // version 3
    x509.set_pubkey(&pkey)?;

    // Without this extension, it is no CA.
    let ca_extension = BasicConstraints::new().critical().ca().build()?;
    x509.append_extension(ca_extension)?;
    let context = x509.x509v3_context(None, None);
    let key_identifier = SubjectKeyIdentifier::new().build(&context)?;
    x509.append_extension(key_identifier)?;

    let serial = generate_serial_number()?;
    x509.set_serial_number(&serial)?;

    x509.sign(&pkey, MessageDigest::sha512())?;

    Ok((x509.build(), pkey))
}

fn load_ca_cert(theme: &'_ dyn Theme, ca: &mut Option<CA>) {
    let validate_path_exists = |input: &str| -> Result<(), &str> {
        if Path::new(input).is_file() {
            Ok(())
        } else {
            Err("File does not exist at given path!")
        }
    };

    let cert = loop {
        let ca_cert_path = Input::<String>::with_theme(theme)
            .with_prompt("Please enter the path to your CA certificate (PEM format):")
            .validate_with(validate_path_exists)
            .interact()
            .unwrap();
        let cert_content = match fs::read(&ca_cert_path) {
            Ok(content) => content,
            Err(err) => {
                println!("Could not read {}: {}", ca_cert_path, err);
                continue;
            }
        };
        match X509::from_pem(&cert_content) {
            Ok(ca_cert) => break ca_cert,
            Err(err) => {
                println!("Could not read CA certificate: {}", err);
                continue;
            }
        }
    };

    let pkey = loop {
        let ca_key_path = Input::<String>::with_theme(theme)
            .with_prompt("Please enter the path to your CA private key:")
            .validate_with(validate_path_exists)
            .interact()
            .unwrap();
        let key_content = match fs::read(&ca_key_path) {
            Ok(content) => content,
            Err(err) => {
                println!("Could not read {}: {}", ca_key_path, err);
                continue;
            }
        };

        let password = Password::with_theme(theme)
            .with_prompt("Please enter the password for your CA private key:")
            .interact()
            .unwrap();

        match PKey::private_key_from_pem_passphrase(&key_content, password.as_bytes()) {
            Ok(key) => {
                break key;
            }
            Err(err) => {
                println!("Could not decode private key: {}", err);
                continue;
            }
        };
    };

    *ca = Some(CA {
        cert,
        pkey,
        created: false,
    });
}

fn create_rpu_cert(ca: &CA, rpu: &Account) -> Result<(X509, PKey<Private>), ErrorStack> {
    if let AccountType::RPU {
        peer_address,
        turi_address,
    } = &rpu.account_type
    {
        println!("Creating Certificate for RPU {}.", rpu.name);

        // FIXME: 365 hardcoded is somehow stupid.
        let valid_for = 365;
        let days = Duration::from_secs(60 * 60 * 24 * (valid_for as u64));
        let info = SystemTime::now().checked_add(days).unwrap();
        println!(
            "The certificate for RPU {} is valid until: {}.",
            rpu.name,
            humantime::format_rfc3339(info)
        );

        let valid_for = Asn1Time::days_from_now(valid_for)?;
        let now = Asn1Time::days_from_now(0)?;

        let pkey = generate_private_key()?;

        let country = entry_from_subject(ca.cert.subject_name(), Nid::COUNTRYNAME);
        let state = entry_from_subject(ca.cert.subject_name(), Nid::STATEORPROVINCENAME);
        let organization = entry_from_subject(ca.cert.subject_name(), Nid::ORGANIZATIONNAME);
        let mut x509_name = X509NameBuilder::new()?;
        x509_name.append_entry_by_nid(Nid::COUNTRYNAME, &country)?;
        x509_name.append_entry_by_nid(Nid::STATEORPROVINCENAME, &state)?;
        x509_name.append_entry_by_nid(Nid::ORGANIZATIONNAME, &organization)?;
        x509_name.append_entry_by_nid(Nid::COMMONNAME, &rpu.name)?;
        let x509_name = x509_name.build();

        let mut x509 = X509::builder()?;
        x509.set_not_after(&valid_for)?;
        x509.set_not_before(&now)?;
        x509.set_subject_name(&x509_name)?;
        x509.set_issuer_name(ca.cert.subject_name())?;
        x509.set_version(2)?; // version 3
        x509.set_pubkey(&pkey)?;

        let serial = generate_serial_number()?;
        x509.set_serial_number(&serial)?;

        // Set all alternative names and extensions.
        let mut alternative_names = SubjectAlternativeName::new();
        alternative_names.dns(&rpu.name);
        let names: HashSet<_> = vec![turi_address, peer_address]
            .iter()
            .map(|address| {
                let hostname_end = address.find(":").unwrap_or(address.len());
                &address[0..hostname_end]
            })
            .collect();
        for name in names {
            if let Ok(ip) = name.parse::<IpAddr>() {
                alternative_names.ip(&ip.to_string());
            } else {
                alternative_names.dns(name);
            }
        }
        let alternative_names = alternative_names
            .build(&x509.x509v3_context(None, None))
            .unwrap();
        x509.append_extension(alternative_names)?;

        // Create extension for using as TLS.
        let key_usage_extension = KeyUsage::new()
            .digital_signature()
            .non_repudiation()
            .key_encipherment()
            .data_encipherment()
            .build()?;
        x509.append_extension(key_usage_extension)?;
        let extended_key_usage_extension = ExtendedKeyUsage::new().server_auth().build()?;
        x509.append_extension(extended_key_usage_extension)?;

        // This is not a CA!
        let ca_extension = BasicConstraints::new().build()?;
        x509.append_extension(ca_extension)?;
        let key_identifier = SubjectKeyIdentifier::new().build(&x509.x509v3_context(None, None))?;
        x509.append_extension(key_identifier)?;

        x509.sign(&ca.pkey, MessageDigest::sha512())?;

        Ok((x509.build(), pkey))
    } else {
        panic!("{} was no RPU.", rpu.name);
    }
}

fn entry_from_subject(name_ref: &X509NameRef, nid: Nid) -> &str {
    std::str::from_utf8(
        name_ref
            .entries_by_nid(nid)
            .next()
            .unwrap()
            .data()
            .as_slice(),
    )
    .unwrap()
}

fn generate_private_key() -> Result<PKey<Private>, ErrorStack> {
    let group = EcGroup::from_curve_name(Nid::SECP384R1)?;
    let key = EcKey::generate(&group)?;
    PKey::from_ec_key(key)
}

fn generate_serial_number() -> Result<Asn1Integer, ErrorStack> {
    // Must be unqiue for each certificate of this authority.
    // But selecting a random number should be sufficient in this case.
    let mut serial = BigNum::new()?;
    serial.rand(8 * 20, MsbOption::MAYBE_ZERO, true)?;
    serial.set_negative(false);
    serial.to_asn1_integer()
}
